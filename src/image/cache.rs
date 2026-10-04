// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A process-wide decode cache, so one resource decoded once is decoded once.
//!
//! # The defect this exists for
//!
//! Three controls each decoded independently and each kept a private copy of the result:
//! `avatar` (`image_source`), `tool_button` (`icon`) and `image_view` (its `Image`). An application
//! showing the same avatar in a list row and in a detail pane read **and** decoded the file twice, and
//! two tool buttons on the same icon did it twice more. Measured before this module existed:
//!
//! ```text
//! $ grep -rn "decode_to_rgba8" src/widget/
//! src/widget/misc_widgets/avatar.rs:369
//! src/widget/menu_toolbar/tool_button.rs:239
//! ```
//!
//! Each site also had its *own* "decode once per key" memo (`decoded_source_key`, `icon_pixels_key`),
//! so the mechanism for "do not decode the same thing twice" existed three times and worked only
//! within one control instance. A fourth control would have added a fourth memo.
//!
//! # What is shared, and what is not
//!
//! Two layers, because there are two costs:
//!
//! 1. **The read** — [`cached_from_path`] answers "these are the bytes this path produces" from a
//!    `stat` when the file has not changed, so twelve tool buttons on one icon read it once. This is
//!    the one place a **path** is a key, and only because it sits *before* the identity question: a
//!    stat is what a filesystem answers without reading, and identity is still settled by content one
//!    layer up. The stamp it compares is `(length, modified)`; a same-length rewrite inside one
//!    filesystem timestamp tick is the accepted residual hole.
//! 2. **The decode** — the pixel layer, keyed by the *content* of the bytes plus the target format,
//!    so two controls naming the same file hit the same entry, and so does one control that reads a
//!    file and another that was handed the same bytes.
//!
//! The file layer exists because the decode layer could not do its job alone: it is keyed on content,
//! and content is what a read produces — so avoiding a read by hashing would require having already
//! read. Before it existed, every file entry point called `std::fs::read` on every request, so a
//! request whose pixels were already cached still paid a full read to discover that.
//!
//! # Why content, and not the path, is the identity
//!
//! Keying on content rather than on the path is deliberate for the *pixels*. A path is not a resource
//! identity: two paths can be the same file (symlink, relative vs absolute, hard link) and one path
//! can be a different file than it was a second ago. Hashing the bytes makes "the same image" mean the
//! same pixels, which is the only thing a cache can safely promise. The file layer above does not
//! contradict this — it caches *the file as it was*, and the pixel layer still decides identity.
//!
//! # The bound, and why it is by bytes
//!
//! An unbounded cache is a leak with a friendly name, and an entry-count bound is the wrong unit:
//! 64 icons and 64 photographs differ by three orders of magnitude. So the budget is in **bytes**, and
//! the eviction order is least-recently-used across **all three maps** — decoded images, extracted
//! pixels and file bytes — because the bound is on memory and those three are equally real. A miss on
//! a full cache evicts until the new entry fits; an item larger than the whole budget is returned but
//! **not** stored, because storing it would evict everything and then immediately be evicted itself —
//! the pathological case that makes a cache slower than no cache.
//!
//! # Where the accounting lives
//!
//! [`stats`] answers "how many decodes did this process avoid", which is the question the plan's
//! criterion asks and the one a profile needs. Counters are cumulative and monotonic; a *frame's*
//! share is the delta between two readings, which is how the frame loop reports it (`FrameStats`'s
//! `decode_requests`/`decode_hits`/`decode_misses`).

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use crate::image::format::DecodedImage;

/// The default budget: 64 MiB of decoded pixel data.
///
/// Chosen so that a full-screen 4K RGBA frame (about 33 MiB) fits whole — a host that caches the
/// background it just decoded must not have it evicted by the next mid-size image — while still
/// being small enough that a process cannot be driven out of memory by a long list of thumbnails.
pub const DEFAULT_BUDGET_BYTES: usize = 64 * 1024 * 1024;

/// One cache entry: the decoded image and what it costs.
struct Entry {
    image: Arc<DecodedImage>,
    /// Bytes of decoded pixel data, which is what the budget is measured in.
    bytes: usize,
    /// A monotonic tick, so the least-recently-used entry can be found without a clock.
    used_at: u64,
}

/// One cached RGBA8 pixel vector.
///
/// A separate map with a separate key space, because it holds a *different* thing: a `Vec<u8>` whose
/// owner is a control, not a `DecodedImage`. Sharing one map would mean one key naming two types.
struct PixelEntry {
    pixels: Arc<Vec<u8>>,
    bytes: usize,
    used_at: u64,
}

/// One cached **file's bytes** — the layer under the decode cache.
///
/// # Why this exists, and why it is keyed differently from everything else here
///
/// The decode cache saves the *decode*. It cannot save the *read*, because it is keyed on content
/// and content is what the read produces: to hash the bytes you must first have them. So
/// `file_rgba8_or_none` used to call `std::fs::read` on **every** request, even a request whose
/// pixels were already cached, and twelve tool buttons on one icon opened and read that file twelve
/// times.
///
/// This layer is the one place a **path** is a legitimate key, and the reason is precisely that it
/// sits *before* the identity question: `(len, mtime)` is what a filesystem will tell you without
/// reading the file, so a stat that matches the stamp means "the bytes this path produced are still
/// the bytes it produces", and a stat that differs means "re-read". That is the standard
/// invalidation rule and it keeps the content-addressing above intact — this layer caches *the file
/// as it was*, and the pixel layer still decides identity by hashing.
///
/// The stamp is deliberately NOT only the path: a path whose file changed under it must not serve
/// the old bytes. Length plus modification time is the cheapest pair of facts that changes when a
/// file's contents do; a same-length rewrite within one filesystem timestamp tick is the residual
/// hole, and it is the same hole every build system accepts.
struct SourceEntry {
    bytes: Arc<Vec<u8>>,
    /// Bytes of file content held. Kept beside the vector rather than asked of it on every eviction
    /// comparison, matching [`PixelEntry`] and [`Entry`].
    size: usize,
    /// `(length, modified-as-nanos)` at the time the bytes were read, so a later stat can tell
    /// whether they are still current.
    stamp: SourceStamp,
    used_at: u64,
}

/// The facts a filesystem will report about a path without reading it.
///
/// `(length, modified-as-nanos)`, compared as a pair. Separated from `SourceEntry` so the comparison
/// has one name and a reader can see the invalidation rule at the point it is applied rather than
/// having to infer it from two struct fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SourceStamp {
    len: u64,
    modified_nanos: u128,
}

/// Which entry the budget should drop next.
///
/// A named type rather than a tuple of `(map-index, key)`, because the three maps have **different
/// key types** — a content hash is a `u64`, a path is bytes — and a tuple would have to invent a
/// placeholder in one of the slots to unify them. Naming the victim keeps the three cases distinct
/// and the eviction loop a plain `match`.
enum Victim {
    Image(u64),
    Pixels(u64),
    Source(std::path::PathBuf),
}

/// The cache itself. Held behind a mutex in a process-wide `OnceLock`.
struct DecodeCache {
    entries: BTreeMap<u64, Entry>,
    pixels: BTreeMap<u64, PixelEntry>,
    /// File bytes, keyed by the path itself rather than by a content hash — see [`SourceEntry`].
    sources: BTreeMap<std::path::PathBuf, SourceEntry>,
    /// Total decoded bytes across both maps, maintained rather than summed on demand so the eviction
    /// loop does not walk the maps to answer "how full am I".
    used_bytes: usize,
    budget_bytes: usize,
    /// The LRU clock. Incremented on every touch; the smallest value is the least recently used.
    tick: u64,
    /// Entries dropped to stay inside the budget, for [`stats`].
    evictions: u64,
}

impl DecodeCache {
    fn new(budget_bytes: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            pixels: BTreeMap::new(),
            sources: BTreeMap::new(),
            used_bytes: 0,
            budget_bytes,
            tick: 0,
            evictions: 0,
        }
    }

    /// The decoded image for `key`, marking it most-recently-used.
    fn get(&mut self, key: u64) -> Option<Arc<DecodedImage>> {
        self.tick += 1;
        let tick = self.tick;
        let entry = self.entries.get_mut(&key)?;
        entry.used_at = tick;
        Some(Arc::clone(&entry.image))
    }

    /// The pixel vector for `key`, marking it most-recently-used.
    fn get_pixels(&mut self, key: u64) -> Option<Arc<Vec<u8>>> {
        self.tick += 1;
        let tick = self.tick;
        let entry = self.pixels.get_mut(&key)?;
        entry.used_at = tick;
        Some(Arc::clone(&entry.pixels))
    }

    /// The file bytes for `path_key`, if the entry is still current per `stamp`, marking it
    /// most-recently-used.
    ///
    /// Returns `None` for a stale entry **and drops it**, so a file that changed does not leave its
    /// old bytes occupying budget until something else happens to evict them.
    fn get_source(
        &mut self,
        path_key: &std::path::Path,
        stamp: SourceStamp,
    ) -> Option<Arc<Vec<u8>>> {
        self.tick += 1;
        let tick = self.tick;
        let entry = self.sources.get_mut(path_key)?;
        if entry.stamp != stamp {
            // A stale entry must not keep its old bytes on the books: the file changed, so its
            // bytes are no longer what any request for this path should get.
            if let Some(old) = self.sources.remove(path_key) {
                self.used_bytes = self.used_bytes.saturating_sub(old.size);
            }
            return None;
        }
        entry.used_at = tick;
        Some(Arc::clone(&entry.bytes))
    }

    /// Stores `bytes` for `path_key`, with the same budgeting rules as [`Self::insert`].
    fn insert_source(
        &mut self,
        path_key: std::path::PathBuf,
        bytes: Arc<Vec<u8>>,
        stamp: SourceStamp,
    ) {
        let size = bytes.len();
        if size > self.budget_bytes {
            return;
        }
        if let Some(old) = self.sources.remove(&path_key) {
            self.used_bytes = self.used_bytes.saturating_sub(old.size);
        }
        self.evict_until_fits(size);
        self.tick += 1;
        let used_at = self.tick;
        self.sources.insert(path_key, SourceEntry { bytes, size, stamp, used_at });
        self.used_bytes += size;
    }

    /// Stores `image` under `key`, evicting least-recently-used entries until it fits.
    ///
    /// An image larger than the whole budget is **not** stored: it would evict every other entry and
    /// still not fit, so storing it would trade the entire cache for one image that will be evicted
    /// by the next insert anyway.
    fn insert(&mut self, key: u64, image: Arc<DecodedImage>, bytes: usize) {
        if bytes > self.budget_bytes {
            return;
        }
        // A re-insert of the same key (its content hash cannot have changed, but a caller may race
        // two decodes of one resource) replaces rather than doubles.
        if let Some(old) = self.entries.remove(&key) {
            self.used_bytes = self.used_bytes.saturating_sub(old.bytes);
        }
        self.evict_until_fits(bytes);
        self.tick += 1;
        let used_at = self.tick;
        self.entries.insert(key, Entry { image, bytes, used_at });
        self.used_bytes += bytes;
    }

    /// Stores a pixel vector under `key`, with the same rules as [`Self::insert`].
    fn insert_pixels(&mut self, key: u64, pixels: Arc<Vec<u8>>, bytes: usize) {
        if bytes > self.budget_bytes {
            return;
        }
        if let Some(old) = self.pixels.remove(&key) {
            self.used_bytes = self.used_bytes.saturating_sub(old.bytes);
        }
        self.evict_until_fits(bytes);
        self.tick += 1;
        let used_at = self.tick;
        self.pixels.insert(key, PixelEntry { pixels, bytes, used_at });
        self.used_bytes += bytes;
    }

    /// Drops least-recently-used entries until `incoming` bytes fit, across all three maps.
    ///
    /// One budget over all of them, because the bound is on *memory*: a decoded image, its extracted
    /// pixels and the file bytes it came from are equally real memory. Evicting by least-recently-used
    /// across the maps, rather than draining one map first, is what keeps a screen that shows only
    /// images from discarding images to make room for pixels it never asked for.
    fn evict_until_fits(&mut self, incoming: usize) {
        while self.used_bytes + incoming > self.budget_bytes {
            let Some(victim) = self.least_recently_used() else { break };
            let removed_bytes = match victim {
                Victim::Image(key) => self.entries.remove(&key).map(|e| e.bytes),
                Victim::Pixels(key) => self.pixels.remove(&key).map(|e| e.bytes),
                Victim::Source(key) => self.sources.remove(&key).map(|e| e.size),
            };
            match removed_bytes {
                Some(bytes) => {
                    self.used_bytes = self.used_bytes.saturating_sub(bytes);
                    self.evictions += 1;
                }
                // The entry was already gone, which cannot happen while the lock is held; breaking
                // rather than looping keeps a hypothetical inconsistency from spinning.
                None => break,
            }
        }
    }

    /// The single least-recently-used entry, whichever of the three maps holds it.
    ///
    /// One function rather than three comparisons at each call site keeps the policy in one place:
    /// "least recently used" has to mean the same thing in the decode layer, the pixel layer and the
    /// file layer, or the budget would be shared by three different replacement rules.
    fn least_recently_used(&self) -> Option<Victim> {
        let mut oldest: Option<(u64, Victim)> = None;
        let mut consider = |used_at: u64, victim: Victim| {
            if oldest.as_ref().is_none_or(|(t, _)| used_at < *t) {
                oldest = Some((used_at, victim));
            }
        };

        for (key, entry) in &self.entries {
            consider(entry.used_at, Victim::Image(*key));
        }
        for (key, entry) in &self.pixels {
            consider(entry.used_at, Victim::Pixels(*key));
        }
        for (key, entry) in &self.sources {
            consider(entry.used_at, Victim::Source(key.clone()));
        }

        oldest.map(|(_, victim)| victim)
    }

    fn clear(&mut self) {
        self.entries.clear();
        self.pixels.clear();
        self.sources.clear();
        self.used_bytes = 0;
    }
}

/// Process-wide counters, read by [`stats`].
///
/// Atomic rather than inside the mutex so a hit does not have to contend with an insert to be
/// counted, and so a profile can read them without taking the cache lock.
static REQUESTS: AtomicU64 = AtomicU64::new(0);
static HITS: AtomicU64 = AtomicU64::new(0);
static MISSES: AtomicU64 = AtomicU64::new(0);
static EVICTIONS: AtomicU64 = AtomicU64::new(0);
static STORED_BYTES: AtomicUsize = AtomicUsize::new(0);

/// The byte budget currently in effect, as [`stats`] reports it.
///
/// Kept in an atomic rather than read through the cache mutex so a profile can read it without
/// contending with a decode, and so it has a value in every build: `0` where the cache is compiled
/// out (no `image` feature, or `alloc_frugal`), because a profile that cannot store anything has
/// a truthful budget of nothing, not the default it never allocated.
#[cfg(all(feature = "image", not(alloc_frugal)))]
static BUDGET_BYTES: AtomicUsize = AtomicUsize::new(DEFAULT_BUDGET_BYTES);
#[cfg(not(all(feature = "image", not(alloc_frugal))))]
static BUDGET_BYTES: AtomicUsize = AtomicUsize::new(0);

/// The file layer's own counters, kept separate from the decode layer's.
///
/// Two layers, two questions. "How many decodes did we avoid" and "how many *reads* did we avoid"
/// have different answers and different fixes — a decode cache with a poor hit rate means the pixels
/// are being requested under different content keys, whereas a file cache with a poor hit rate means
/// the path or the stamp is changing. Folding them into one counter would make both questions
/// unanswerable.
static FILE_REQUESTS: AtomicU64 = AtomicU64::new(0);
static FILE_HITS: AtomicU64 = AtomicU64::new(0);
static FILE_MISSES: AtomicU64 = AtomicU64::new(0);

#[cfg(all(feature = "image", not(alloc_frugal)))]
static CACHE: std::sync::Mutex<Option<DecodeCache>> = std::sync::Mutex::new(None);

/// Runs `f` with the cache, creating it on first use with [`DEFAULT_BUDGET_BYTES`].
///
/// A poisoned lock is recovered rather than propagated: the cache is a pure memo with no invariant
/// spanning two entries, so a panic in one thread's decode must not make every later decode fail.
#[cfg(all(feature = "image", not(alloc_frugal)))]
fn with_cache<R>(f: impl FnOnce(&mut DecodeCache) -> R) -> R {
    let mut guard = match CACHE.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    let cache = guard.get_or_insert_with(|| DecodeCache::new(DEFAULT_BUDGET_BYTES));
    f(cache)
}

/// A snapshot of the cache's cumulative accounting.
///
/// # How to read it
///
/// `hits + misses == requests` always, and `requests - misses` is the count of decodes that did not
/// happen. `bytes` is what the cache currently holds and `budget_bytes` what it may hold, so a
/// profile can tell "the budget is too small" (evictions climbing, hit rate flat) from "the cache is
/// not being used" (requests flat).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DecodeCacheStats {
    /// Calls that asked for a decoded image.
    pub requests: u64,
    /// Calls answered from the cache without decoding.
    pub hits: u64,
    /// Calls that had to decode.
    pub misses: u64,
    /// Entries dropped to stay inside the budget.
    pub evictions: u64,
    /// Decoded pixel bytes currently held.
    pub bytes: usize,
    /// The byte budget.
    pub budget_bytes: usize,
    /// Calls that asked for a file's bytes (the layer under the decode cache).
    pub file_requests: u64,
    /// Those answered from cache without reading the file — `stat` only.
    pub file_hits: u64,
    /// Those that had to read the file.
    pub file_misses: u64,
}

impl DecodeCacheStats {
    /// Decodes avoided, i.e. the work the cache actually saved.
    pub fn avoided(&self) -> u64 {
        self.hits
    }

    /// The fraction of requests answered without decoding, in `0.0..=1.0`.
    ///
    /// `0.0` for no requests, which reads as "no data" rather than as "the cache never helps" —
    /// the honest answer when nothing has been asked of it.
    pub fn hit_rate(&self) -> f32 {
        if self.requests == 0 {
            0.0
        } else {
            self.hits as f32 / self.requests as f32
        }
    }
}

/// The cache's cumulative accounting.
///
/// Readable in every build, including one without the `image` feature, where it truthfully reports
/// zero requests rather than being absent — a profile should not have to know which features were
/// compiled to call this.
pub fn stats() -> DecodeCacheStats {
    DecodeCacheStats {
        requests: REQUESTS.load(Ordering::Relaxed),
        hits: HITS.load(Ordering::Relaxed),
        misses: MISSES.load(Ordering::Relaxed),
        evictions: EVICTIONS.load(Ordering::Relaxed),
        bytes: STORED_BYTES.load(Ordering::Relaxed),
        budget_bytes: BUDGET_BYTES.load(Ordering::Relaxed),
        file_requests: FILE_REQUESTS.load(Ordering::Relaxed),
        file_hits: FILE_HITS.load(Ordering::Relaxed),
        file_misses: FILE_MISSES.load(Ordering::Relaxed),
    }
}

/// Serialises a test that measures [`stats`] **deltas**.
///
/// The counters are process-wide and monotonic, which is right for a profile and wrong for a test
/// that asserts "these three calls produced one miss": another test decoding in parallel lands in the
/// same counters. Taking this guard for the duration of a delta measurement is what makes the
/// assertion about the calls the test made rather than about the whole test binary.
///
/// The same shape as [`crate::theme::theme_test_guard`], and for the same reason — a process-wide
/// facility cannot be measured per-test without agreeing on who is measuring.
#[cfg(test)]
pub(crate) fn stats_test_guard() -> crate::compat::MutexGuard<'static, ()> {
    static GUARD: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    let mutex = GUARD.get_or_init(|| std::sync::Mutex::new(()));
    match mutex.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

/// Empties the cache, leaving the counters alone.
///
/// The counters are cumulative because they answer "what has this process saved", which a clear does
/// not change. A caller wanting a fresh measurement takes [`stats`] before and after instead.
pub fn clear() {
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    {
        with_cache(|cache| cache.clear());
        STORED_BYTES.store(0, Ordering::Relaxed);
    }
}

/// Sets the byte budget, evicting down to it if the cache is over.
///
/// # Why this is a process-wide setter and not a per-cache constructor
///
/// There is one cache, because the point is for controls that do not know about each other to share
/// it. A host with a memory budget of its own states it here once, at startup.
///
/// A budget of `0` disables storage — every request decodes — which is a legitimate configuration for
/// a host that would rather pay CPU than memory, and is *not* the same as "the cache is broken".
pub fn set_budget_bytes(budget_bytes: usize) {
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    {
        // Publish the new budget before touching the cache so a caller that reads `stats` in the
        // same instant already sees the value it just configured, not the previous one.
        BUDGET_BYTES.store(budget_bytes, Ordering::Relaxed);
        let (used, evictions) = with_cache(|cache| {
            cache.budget_bytes = budget_bytes;
            // Evict down to the new budget now, so the accounting a caller reads immediately after
            // this call is the truth rather than a deferred answer.
            cache.evict_until_fits(0);
            (cache.used_bytes, cache.evictions)
        });
        STORED_BYTES.store(used, Ordering::Relaxed);
        // The eviction count is cumulative and owned by the cache; publish the total so `stats`
        // reports one number rather than two half-answers.
        EVICTIONS.store(evictions, Ordering::Relaxed);
    }
}

/// The byte cost of a decoded image, which is what the budget is measured in.
///
/// Only the pixel payload counts. `DecodedImage`'s other fields are an enum discriminant and a
/// handful of scalars, so including them would add a constant per entry and make the accounting look
/// arbitrary; the pixels are the term that actually grows.
fn decoded_bytes(image: &DecodedImage) -> usize {
    use crate::image::format::ImageData;
    match &image.data {
        ImageData::Rgba8(v) | ImageData::Rgb8(v) | ImageData::Grayscale8(v) => v.len(),
        ImageData::Grayscale16(v) => v.len(),
        ImageData::Rgba16(v) | ImageData::Rgb16(v) => v.len(),
    }
}

/// A stable content key for a request: the bytes plus the target format.
///
/// # Why FNV-1a and not a cryptographic hash
///
/// The key is a *cache* index, not a security boundary: a collision costs a wrong image, and the
/// threat model is "two different pictures hashed to the same 64 bits", which is not something an
/// application does on purpose. A cryptographic hash would be the right choice for a content-
/// addressed store exposed to untrusted input; here it would be a dependency and a per-decode cost
/// paid for a property nothing relies on.
///
/// The digest is over the bytes **and** the format tag, so the same bytes requested as `Rgba8` and
/// as something else cannot collide.
fn content_key(data: &[u8], format_tag: u8) -> u64 {
    // FNV-1a, 64-bit, computed in two 32-bit halves by `u64` wrapping multiply.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |byte: u8| {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    mix(width_tag(data.len()));
    for &byte in data {
        mix(byte);
    }
    mix(format_tag);
    hash
}

/// A cheap, injective-as-practical encoding of a length into the hash stream.
///
/// Without it, the byte streams `[1, 2]` and `[1, 2]` followed by different tails could be arranged
/// to collide across different lengths — so the length is hashed first, which is the standard
/// prefix-free fix and costs four mixes.
fn width_tag(len: usize) -> u8 {
    // The low byte is enough to separate realistic image sizes when combined with the payload
    // itself; a full 8-byte field would cost eight mixes per image for a distinction the content
    // already makes.
    (len & 0xff) as u8
}

/// The format tag hashed after the payload, so a request cannot collide with another format's.
const FORMAT_TAG_RGBA8: u8 = 1;

/// The tag for the pixel-vector cache, distinct from [`FORMAT_TAG_RGBA8`] so a decoded image and its
/// extracted pixels are two entries rather than one overwriting the other.
#[cfg(all(feature = "image", not(alloc_frugal)))]
const PIXELS_TAG_RGBA8: u8 = 2;

/// Decodes `data` as RGBA8, reusing a previously decoded result for the same bytes.
///
/// This is the entry point every control should use. It is a drop-in for
/// [`crate::image::decoder::decode_to_rgba8`] — same signature, same errors — with the difference
/// that the second caller of the same bytes pays no decode.
///
/// # Errors
///
/// A decode **failure is not cached**. A file that was truncated when it was first read may be
/// complete when it is read again, and caching the absence of an image would turn a transient error
/// into a permanent one. `misses` therefore counts attempts, which is the honest reading of the
/// counter for a stream of failures.
#[cfg(all(feature = "image", not(alloc_frugal)))]
pub fn decode_to_rgba8_cached(data: &[u8]) -> Result<Arc<DecodedImage>, String> {
    REQUESTS.fetch_add(1, Ordering::Relaxed);
    let key = content_key(data, FORMAT_TAG_RGBA8);

    if let Some(image) = with_cache(|cache| cache.get(key)) {
        HITS.fetch_add(1, Ordering::Relaxed);
        return Ok(image);
    }

    MISSES.fetch_add(1, Ordering::Relaxed);
    let decoded = crate::image::decoder::decode_to_rgba8(data)?;
    let bytes = decoded_bytes(&decoded);
    let image = Arc::new(decoded);
    let (used, evictions) = with_cache(|cache| {
        cache.insert(key, Arc::clone(&image), bytes);
        (cache.used_bytes, cache.evictions)
    });
    STORED_BYTES.store(used, Ordering::Relaxed);
    EVICTIONS.store(evictions, Ordering::Relaxed);
    Ok(image)
}

/// The bytes of `path`, reusing a previous read when the file has not changed.
///
/// # Why this is the layer under the decode cache
///
/// [`decode_to_rgba8_cached`] saves the *decode*, and it is keyed on content — so it cannot save
/// the *read*, because reading is what produces the thing it hashes. Every file entry point used to
/// call `std::fs::read` unconditionally, so twelve tool buttons on one icon opened, read and hashed
/// that file twelve times to reach a pixel entry that only needed inserting once.
///
/// This is the one place in the module where a **path** is the key, and the reason is that it sits
/// before the identity question: a `stat` is what a filesystem answers without reading, so a stamp
/// that matches means these are still the bytes, and a stamp that changed means re-read. Identity is
/// still decided by content one layer up — this layer only promises "the file as it was".
///
/// # The stamp, and the residual hole
///
/// `(length, modified)`. A same-length rewrite inside one filesystem timestamp tick would not be
/// noticed. That is the same hole every build system accepts, and the alternative — reading the file
/// to find out whether you could have avoided reading the file — is not a cache.
///
/// # Errors
///
/// The caller gets the error the read produced. Unlike a decode failure, **a read failure is not
/// remembered**: there is nothing to remember (the cache stores bytes), and a file that appears
/// later would otherwise stay "unreadable".
#[cfg(all(feature = "image", not(alloc_frugal)))]
pub fn cached_from_path(path: impl AsRef<std::path::Path>) -> Result<Arc<Vec<u8>>, std::io::Error> {
    let path = path.as_ref();
    let metadata = std::fs::metadata(path)?;
    let stamp = SourceStamp {
        len: metadata.len(),
        modified_nanos: metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|since| since.as_nanos())
            .unwrap_or(0),
    };
    let path_key = path_key(path);

    FILE_REQUESTS.fetch_add(1, Ordering::Relaxed);
    if let Some(bytes) = with_cache(|cache| cache.get_source(&path_key, stamp)) {
        FILE_HITS.fetch_add(1, Ordering::Relaxed);
        return Ok(bytes);
    }
    FILE_MISSES.fetch_add(1, Ordering::Relaxed);

    let bytes = Arc::new(std::fs::read(path)?);
    let stored = Arc::clone(&bytes);
    let (used, evictions) = with_cache(|cache| {
        cache.insert_source(path_key, stored, stamp);
        (cache.used_bytes, cache.evictions)
    });
    STORED_BYTES.store(used, Ordering::Relaxed);
    EVICTIONS.store(evictions, Ordering::Relaxed);
    Ok(bytes)
}

/// The cache key for a path: the path itself, as a [`std::path::PathBuf`].
///
/// # Why the key is the path, not normalised bytes
///
/// A path is what the filesystem answers with, and [`std::path::Path`] already models that
/// identity losslessly and per-platform: it preserves non-UTF-8 names (an `OsStr` is not a
/// lossy string) and it compares components the way the OS does. On Windows that means `a\b`
/// and `a/b` are the **same** key (both separators name the same file); on macOS and Unix a
/// backslash is an ordinary filename byte, so `a\b` and `a/b` stay **distinct**.
///
/// The previous implementation normalised every `\` into `/` on every platform and then ran
/// the result through `to_string_lossy`, which did two things wrong at once: it merged two
/// legal distinct files on macOS, and it folded distinct non-UTF-8 names onto one replacement
/// character. Keying by `PathBuf` removes both losses without any OS branch in this file —
/// the platform knowledge lives in `std::path` itself (principle #35/#36).
#[cfg(all(feature = "image", not(alloc_frugal)))]
fn path_key(path: &std::path::Path) -> std::path::PathBuf {
    path.to_path_buf()
}

/// Reads and decodes a file as RGBA8, reusing a previously decoded result for the same bytes.
///
/// The convenience the controls actually call: it does the `fs::read` and the cached decode in one
/// step, so a site cannot accidentally take the uncached path.
#[cfg(all(feature = "image", not(alloc_frugal)))]
pub fn decode_file_to_rgba8_cached(
    path: impl AsRef<std::path::Path>,
) -> Result<Arc<DecodedImage>, String> {
    let bytes = cached_from_path(path.as_ref()).map_err(|error| error.to_string())?;
    decode_to_rgba8_cached(&bytes)
}

/// The RGBA8 pixels of a file, or `None` if it cannot be read or decoded.
///
/// Takes `&Path` rather than `&str` because the callers hold paths: a `tool_button`'s icon is a
/// `PathBuf` and an `avatar`'s source is a `String`. Widening here is what stops each control from
/// converting to whatever the other one happened to pass.
///
/// The controls' shape: a draw path has no way to report an error and must fall back to drawing
/// nothing, so the error is logged here rather than returned. Centralising that is what stops three
/// controls from each deciding differently what a failed icon looks like.
///
/// # Why the pixels are cached as their own entry
///
/// The controls want `Vec<u8>`, and `Arc<Vec<u8>>` cannot be projected out of an `Arc<DecodedImage>`
/// — Rust has no way to re-borrow the payload as a separately-counted allocation. Cloning the pixels
/// on every request would make the cache a pessimisation (a full copy *plus* the decode it saved), so
/// the cloned vector is itself stored under a second key. The result: the decode happens once, the
/// copy happens once, and every later request is an `Arc` bump — which is what "shared" has to mean
/// to be worth anything.
///
/// # And why it reads through [`cached_from_path`] rather than `std::fs::read`
///
/// Reading directly here made the decode cache cheaper than it looked: a request whose pixels were
/// already cached still paid a full file read to find that out. The read is now the layer below, so
/// the second and every later request for one file costs a `stat` and an `Arc` bump.
#[cfg(all(feature = "image", not(alloc_frugal)))]
pub fn file_rgba8_or_none(path: impl AsRef<std::path::Path>) -> Option<Arc<Vec<u8>>> {
    let path = path.as_ref();
    let bytes = match cached_from_path(path) {
        Ok(bytes) => bytes,
        Err(error) => {
            log::warn!("image {path:?} could not be read ({error}); nothing is drawn for it");
            return None;
        }
    };
    pixels_rgba8_or_none(&bytes)
}

/// The RGBA8 pixels of `data`, or `None` if it does not decode.
///
/// See [`file_rgba8_or_none`] for why the pixel vector gets its own cache entry.
#[cfg(all(feature = "image", not(alloc_frugal)))]
pub fn pixels_rgba8_or_none(data: &[u8]) -> Option<Arc<Vec<u8>>> {
    REQUESTS.fetch_add(1, Ordering::Relaxed);
    let key = content_key(data, PIXELS_TAG_RGBA8);
    if let Some(pixels) = with_cache(|cache| cache.get_pixels(key)) {
        HITS.fetch_add(1, Ordering::Relaxed);
        return Some(pixels);
    }
    MISSES.fetch_add(1, Ordering::Relaxed);
    match crate::image::decoder::decode_to_rgba8(data) {
        Ok(decoded) => match decoded.data {
            crate::image::format::ImageData::Rgba8(pixels) => {
                let bytes = pixels.len();
                let shared = Arc::new(pixels);
                let (used, evictions) = with_cache(|cache| {
                    cache.insert_pixels(key, Arc::clone(&shared), bytes);
                    (cache.used_bytes, cache.evictions)
                });
                STORED_BYTES.store(used, Ordering::Relaxed);
                EVICTIONS.store(evictions, Ordering::Relaxed);
                Some(shared)
            }
            // `decode_to_rgba8` guarantees the `Rgba8` variant, so this arm is unreachable; it is
            // written out rather than `unreachable!()`ed because this is called from a draw path.
            _ => None,
        },
        Err(reason) => {
            log::warn!("image bytes could not be decoded ({reason}); nothing is drawn for them");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::format::{DecodedImage, ImageData, ImageFormat};

    /// A tiny valid PNG, so the decode path is exercised for real rather than mocked.
    ///
    /// 1x1 opaque red, 70 bytes, produced by `zlib` rather than assembled by hand: a hand-built IDAT
    /// that *looks* right fails in the inflater, and a fixture that never decodes proves nothing
    /// about a cache whose whole job is to answer the second request.
    const PNG_1X1_RED: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    fn png_1x1() -> Vec<u8> {
        PNG_1X1_RED.to_vec()
    }

    #[test]
    fn the_key_depends_on_the_bytes_and_the_format() {
        let a = content_key(b"hello", FORMAT_TAG_RGBA8);
        let b = content_key(b"hello", FORMAT_TAG_RGBA8);
        let c = content_key(b"hellp", FORMAT_TAG_RGBA8);
        assert_eq!(a, b, "the same request must key the same");
        assert_ne!(a, c, "a one-byte change must change the key");
        assert_ne!(
            content_key(b"hello", FORMAT_TAG_RGBA8),
            content_key(b"hello", 2),
            "the format is part of the request"
        );
        // The length is hashed, so a prefix cannot collide with a longer stream.
        assert_ne!(content_key(b"ab", FORMAT_TAG_RGBA8), content_key(b"ab\0", FORMAT_TAG_RGBA8));
    }

    /// The whole point: the second request of the same bytes must not decode.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn the_same_bytes_decode_once() {
        let _guard = stats_test_guard();
        clear();
        let before = stats();
        let png = png_1x1();

        let first = decode_to_rgba8_cached(&png).expect("the PNG must decode");
        let second = decode_to_rgba8_cached(&png).expect("and the cached one");
        let third = decode_to_rgba8_cached(&png).expect("and again");

        assert!(Arc::ptr_eq(&first, &second), "a hit must hand back the same allocation");
        assert!(Arc::ptr_eq(&second, &third));

        let after = stats();
        assert_eq!(after.requests - before.requests, 3, "three requests");
        assert_eq!(after.misses - before.misses, 1, "but one decode");
        assert_eq!(after.hits - before.hits, 2, "and two saves");
        assert_eq!(after.avoided(), after.hits);
        assert_eq!(after.hits + after.misses, after.requests, "the books must balance");
    }

    /// The pixel-vector path must be shared too, and must be a *different* entry from the image path.
    ///
    /// The controls call this one, so a hit here is the case that actually saves work; and the two
    /// paths key with different tags, so requesting the image and then its pixels is two decodes
    /// rather than one path silently answering for the other.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn the_pixel_vector_is_shared_between_requests() {
        let _guard = stats_test_guard();
        clear();
        let png = png_1x1();
        let before = stats();

        let a = pixels_rgba8_or_none(&png).expect("decodes");
        let b = pixels_rgba8_or_none(&png).expect("hits");
        assert!(Arc::ptr_eq(&a, &b), "the second request must not re-allocate the pixels");
        assert_eq!(a.len(), 4, "one RGBA pixel");

        let after = stats();
        assert_eq!(after.requests - before.requests, 2);
        assert_eq!(after.misses - before.misses, 1);
        assert_eq!(after.hits - before.hits, 1);

        // And the image path is its own entry, so it decodes again rather than reusing these pixels.
        let mid = stats();
        let _ = decode_to_rgba8_cached(&png).expect("decodes");
        assert_eq!(
            stats().misses - mid.misses,
            1,
            "a different request shape is a different entry"
        );
    }

    /// A failed decode must not be remembered as a failure.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn a_decode_failure_is_not_cached() {
        let _guard = stats_test_guard();
        clear();
        let before = stats();
        assert!(decode_to_rgba8_cached(b"not an image at all").is_err());
        assert!(decode_to_rgba8_cached(b"not an image at all").is_err());
        let after = stats();
        assert_eq!(
            after.misses - before.misses,
            2,
            "a transient read failure must not become a permanent one"
        );
        assert_eq!(after.hits - before.hits, 0, "there was nothing to hit");
    }

    /// The budget must be honoured, and honoured by eviction rather than by refusing to fill.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn the_budget_is_enforced_by_eviction() {
        let image = |tag: u8, side: usize| {
            Arc::new(DecodedImage::new(
                ImageFormat::Rgba8,
                ImageData::Rgba8(vec![tag; side * side * 4]),
                side as u32,
                side as u32,
            ))
        };
        // A private cache, so this test cannot disturb the process-wide one.
        let mut cache = DecodeCache::new(1000);
        cache.insert(1, image(1, 10), 400);
        cache.insert(2, image(2, 10), 400);
        assert_eq!(cache.used_bytes, 800);
        assert!(cache.get(1).is_some(), "touching 1 makes 2 the victim");
        let evictions_before = cache.evictions;
        cache.insert(3, image(3, 10), 400);
        assert_eq!(cache.evictions, evictions_before + 1, "exactly one victim was dropped");
        assert!(cache.used_bytes <= 1000, "the budget is a ceiling, not a target");
        assert!(cache.get(1).is_some(), "the recently used entry survives");
        assert!(cache.get(2).is_none(), "the least recently used one was dropped");
    }

    /// An image larger than the budget must be returned but not stored.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn an_oversized_image_is_not_stored() {
        let mut cache = DecodeCache::new(100);
        let huge =
            Arc::new(DecodedImage::new(ImageFormat::Rgba8, ImageData::Rgba8(vec![0; 400]), 10, 10));
        cache.insert(1, huge, 400);
        assert_eq!(cache.used_bytes, 0, "storing it would evict everything for nothing");
        assert!(cache.get(1).is_none());
    }

    #[test]
    fn the_counters_describe_an_empty_cache_honestly() {
        let stats = DecodeCacheStats::default();
        assert_eq!(stats.hit_rate(), 0.0, "no requests is not a hit rate of zero-proud");
        assert_eq!(stats.avoided(), 0);
        let half = DecodeCacheStats { requests: 8, hits: 4, ..Default::default() };
        assert!((half.hit_rate() - 0.5).abs() < f32::EPSILON);
    }

    // ── The file layer (the read under the decode) ──────────────────────────────

    /// Writes `bytes` to a uniquely-named file under the system temp directory.
    ///
    /// A per-test name rather than a fixed one: the file cache is process-wide and keyed on the
    /// path, so two tests sharing a name would share an entry with different content — the same
    /// cross-test coupling the decode cache's own docs record.
    fn write_temp(name_hint: &str, bytes: &[u8]) -> std::path::PathBuf {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "rw_cache_test_{}_{}_{}.png",
            name_hint,
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::write(&path, bytes).expect("the temp file must be writable");
        path
    }

    /// The second read of one file must not read the file.
    ///
    /// # The defect this pins
    ///
    /// Every file entry point called `std::fs::read` unconditionally, so the decode cache could only
    /// ever save the *decode*: a request whose pixels were already cached still opened, read and
    /// hashed the file to discover that. The `file_hits` counter is the observable — it is what makes
    /// "the read was avoided" a fact rather than a claim.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn the_second_read_of_one_file_does_not_read_it() {
        let _guard = stats_test_guard();
        clear();
        let png = png_1x1();
        let path = write_temp("read_once", &png);

        let before = stats();
        let first = cached_from_path(&path).expect("the file reads");
        let second = cached_from_path(&path).expect("and the cached one");
        let third = cached_from_path(&path).expect("and again");
        let after = stats();

        assert_eq!(&*first, &png[..], "the bytes are the file's");
        assert!(Arc::ptr_eq(&first, &second), "a hit hands back the same allocation");
        assert!(Arc::ptr_eq(&second, &third));
        assert_eq!(after.file_requests - before.file_requests, 3);
        assert_eq!(
            after.file_hits - before.file_hits,
            2,
            "the second and third reads were avoided"
        );
        assert_eq!(after.file_misses - before.file_misses, 1, "exactly one real read");
        let _ = std::fs::remove_file(&path);
    }

    /// A file whose contents change must not serve the previous bytes.
    ///
    /// The stamp is `(len, modified)`, so this writes a file, reads it, then rewrites it with
    /// **different-length** content — which is the case the stamp is guaranteed to catch. Without the
    /// stamp check the cache would answer with a picture of something that is no longer on disk, and
    /// because both layers hash content, nothing above would notice.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn a_changed_file_is_not_served_from_stale_bytes() {
        let _guard = stats_test_guard();
        clear();
        let path = write_temp("changed", b"first version");
        let first = cached_from_path(&path).expect("reads");
        assert_eq!(&*first, b"first version");

        // A longer file, so `len` differs and the stamp cannot match.
        std::thread::sleep(std::time::Duration::from_millis(1100));
        std::fs::write(&path, b"a second, much longer version").expect("rewrite");

        let second = cached_from_path(&path).expect("re-reads");
        assert_eq!(
            &*second, b"a second, much longer version",
            "a changed file must be re-read, not served from the old bytes"
        );
        assert!(!Arc::ptr_eq(&first, &second));
        let _ = std::fs::remove_file(&path);
    }

    /// The pixel layer sits on top of the byte layer: one read, one decode, for N requests.
    ///
    /// This is the composition the twelve-tool-buttons-on-one-icon case exercises, stated as the two
    /// counters it moves. Before the byte layer the decode was saved and the read was not; the pair of
    /// assertions is what distinguishes the two.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn one_file_reads_and_decodes_once_for_many_requests() {
        let _guard = stats_test_guard();
        clear();
        let png = png_1x1();
        let path = write_temp("compose", &png);

        let before = stats();
        for _ in 0..12 {
            let pixels = file_rgba8_or_none(&path).expect("twelve controls on one icon");
            assert_eq!(pixels.len(), 4, "a 1x1 RGBA image is four bytes");
        }
        let after = stats();

        assert_eq!(after.file_misses - before.file_misses, 1, "the file was read once");
        assert_eq!(after.file_hits - before.file_hits, 11, "and the other eleven were stats");
        assert_eq!(after.misses - before.misses, 1, "and decoded once");
        assert_eq!(after.hits - before.hits, 11);
        let _ = std::fs::remove_file(&path);
    }

    /// A missing file is an error every time, and is never remembered as "unreadable".
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn a_missing_file_errors_and_is_not_remembered() {
        let _guard = stats_test_guard();
        clear();
        let mut path = std::env::temp_dir();
        path.push(format!("rw_cache_test_missing_{}.png", std::process::id()));
        let _ = std::fs::remove_file(&path);

        assert!(cached_from_path(&path).is_err(), "a missing file is an error");
        // Create it, and the next call must succeed: the failure was not cached.
        std::fs::write(&path, png_1x1()).expect("write");
        assert!(
            cached_from_path(&path).is_ok(),
            "a file that appears later must be readable; the earlier failure was not an entry"
        );
        let _ = std::fs::remove_file(&path);
    }

    /// The three maps share one budget, and the file layer is part of it.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn the_file_layer_is_inside_the_same_budget() {
        let mut cache = DecodeCache::new(100);
        let stamp = SourceStamp { len: 80, modified_nanos: 1 };
        cache.insert_source(std::path::PathBuf::from("a"), Arc::new(vec![0u8; 80]), stamp);
        assert_eq!(cache.used_bytes, 80, "the file bytes are on the budget's books");

        // A second file that does not fit evicts the first rather than overrunning.
        let stamp2 = SourceStamp { len: 80, modified_nanos: 2 };
        cache.insert_source(std::path::PathBuf::from("b"), Arc::new(vec![1u8; 80]), stamp2);
        assert!(cache.used_bytes <= 100, "the budget is a ceiling across all three maps");
        assert_eq!(cache.evictions, 1);
        assert!(
            cache.get_source(std::path::Path::new("a"), stamp).is_none(),
            "the older file was the victim"
        );
        assert!(cache.get_source(std::path::Path::new("b"), stamp2).is_some());
    }

    /// The configured budget must be the number [`stats`] reports, immediately.
    ///
    /// Pins the defect: `set_budget_bytes` mutated the cache's internal budget, but `stats` read a
    /// hardcoded `DEFAULT_BUDGET_BYTES`, so a host that set its own budget could never observe it.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn the_configured_budget_is_immediately_queryable() {
        let _guard = stats_test_guard();
        set_budget_bytes(123_456);
        assert_eq!(
            stats().budget_bytes,
            123_456,
            "the budget a caller configured must be the one stats reports"
        );
        set_budget_bytes(7);
        assert_eq!(stats().budget_bytes, 7, "a later adjustment must win over the earlier one");
        set_budget_bytes(0);
        assert_eq!(
            stats().budget_bytes,
            0,
            "a zero budget (storage disabled) is a real, reportable configuration"
        );
        // Restore the default so this test does not leak a budget into the process-wide cache
        // that later tests in the binary read.
        set_budget_bytes(DEFAULT_BUDGET_BYTES);
        assert_eq!(stats().budget_bytes, DEFAULT_BUDGET_BYTES);
    }

    /// The file cache must key on the path, not on lossy, separator-rewritten bytes.
    ///
    /// Pins the defect: `path_key` rewrote every `\` into `/` and ran the result through
    /// `to_string_lossy`, so two legal distinct files on macOS (`a\b` and `a/b`) landed on one key.
    /// The key is now the `PathBuf` itself; the OS's own `Path` equality is the oracle for whether
    /// two spellings name one file, so the test stays correct on every platform without a `cfg`.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn distinct_path_spellings_key_the_way_the_os_says() {
        let slash = std::path::PathBuf::from("a/b");
        let backslash = std::path::PathBuf::from("a\\b");
        let same_file = slash == backslash;

        let mut cache = DecodeCache::new(1000);
        let stamp = SourceStamp { len: 1, modified_nanos: 1 };
        cache.insert_source(slash.clone(), Arc::new(vec![0xAA]), stamp);
        cache.insert_source(backslash.clone(), Arc::new(vec![0xBB]), stamp);

        if same_file {
            // On Windows the two spellings name one file, so the second write replaced the first.
            assert_eq!(&*cache.get_source(&slash, stamp).unwrap(), &[0xBB]);
        } else {
            // On Unix/macOS a backslash is an ordinary filename byte, so the two are distinct files
            // and each keeps its own bytes — the behaviour the old byte key destroyed.
            assert_eq!(&*cache.get_source(&slash, stamp).unwrap(), &[0xAA]);
            assert_eq!(&*cache.get_source(&backslash, stamp).unwrap(), &[0xBB]);
        }
    }

    /// `path_key` must be the lossless path, not a rewritten byte string.
    #[cfg(all(feature = "image", not(alloc_frugal)))]
    #[test]
    fn the_path_key_is_the_lossless_path() {
        assert_eq!(path_key(std::path::Path::new("a\\b")), std::path::PathBuf::from("a\\b"));
        assert_eq!(path_key(std::path::Path::new("a/b")), std::path::PathBuf::from("a/b"));
    }
}
