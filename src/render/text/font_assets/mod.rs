// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! The generated vector faces, one module per opt-in data feature.
//!
//! # Why each face is a module rather than a constant
//!
//! The payload is a binary `include_bytes!` rather than a Rust array: 36 KB written as `0x00,`
//! literals would be ~150 KB of source, which is a worse artifact than the binary it describes.
//! Each module's header records where the bytes came from and under which licence — see the
//! repository-root `NOTICE`, which `tools/check_font_licenses.sh` requires this to agree with.
//!
//! # Why nothing here is named after the upstream font
//!
//! OFL's Reserved Font Name clause applies to a Modified Version, and a subset is one. The
//! upstream names live on as `covers` strings (a font *faces* an application by its family
//! name, which is a different thing), never as this crate's public identifiers.

/// One available vector face: its bytes, and the name a diagnostic prints.
///
/// # Why this is gated on `text-shaping` as well as the data features
///
/// [`Self`] is the *shaper's* input type, and the shaper is enabled by `text-shaping` — a feature a
/// caller can turn on to supply their own face at runtime, with no generated data at all. Gating
/// this on the data features alone makes `fonts-emoji-color, text-shaping` fail to compile: the
/// shaper module imports this type, and neither feature brings it into existence. A build must be
/// able to enable shaping without shipping glyph data, so the gate is "shaping, or a data feature".
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
))]
#[derive(Clone, Copy)]
pub struct FaceBytes {
    /// The face's family name, for diagnostics and for a caller that asks for it by name.
    pub name: &'static str,
    /// The subset's bytes, read from the binary's read-only section.
    pub bytes: &'static [u8],
}

/// A colour bitmap face: its bytes, and the name a diagnostic prints.
///
/// A distinct type from [`FaceBytes`] on purpose. A colour face has no outlines, so a shaper cannot
/// read it and a rasteriser cannot outline it — conflating the two would let a caller pass a colour
/// face to `RustybuzzShaper` and get silently empty shaping instead of a type error.
#[cfg(feature = "fonts-emoji-color")]
#[derive(Clone, Copy)]
pub struct ColorFaceBytes {
    /// The face's family name, for diagnostics and for a caller that asks for it by name.
    pub name: &'static str,
    /// The subset's bytes, read from the binary's read-only section.
    pub bytes: &'static [u8],
}

#[cfg(feature = "fonts-vector-latin")]
mod latin;
#[cfg(feature = "fonts-vector-latin")]
pub use latin::FONT as LATIN;

#[cfg(feature = "fonts-complex")]
mod arabic;
#[cfg(feature = "fonts-complex")]
pub use arabic::FONT as ARABIC;

// The scalable CJK face (G-4c). Distinct from `fonts-cjk-bitmap`: this one carries **outlines**, so
// it needs the rasteriser and can be drawn at any px size, at the cost of ~7x the bytes for the
// same coverage. See `tools/cjk_vector_codepoints.txt` for why its coverage is narrower.
#[cfg(feature = "fonts-cjk")]
#[cfg_attr(docsrs, doc(cfg(feature = "fonts-cjk")))]
mod cjk;
#[cfg(feature = "fonts-cjk")]
pub use cjk::FONT as CJK;

// The sharded CJK vector face (`fonts-cjk-shards`). Each shard is one `include_bytes!` payload
// behind its own feature; the generated module names the shard a character lives in so a host can
// load one script rather than all of CJK. See `tools/cjk_vector_shards.txt`.
#[cfg(any(
    feature = "fonts-cjk-shard-latin",
    feature = "fonts-cjk-shard-symbols",
    feature = "fonts-cjk-shard-kana",
    feature = "fonts-cjk-shard-fullwidth",
    feature = "fonts-cjk-shard-han"
))]
/// The sharded CJK vector face: the `fonts-cjk` coverage partitioned on Unicode block boundaries.
///
/// Each shard is one `include_bytes!` payload behind its own `fonts-cjk-shard-<id>` feature, and
/// [`cjk_shards::shard_for`] names the shard a character lives in. Use
/// [`cjk_shard_loader::load_shard_for`] to make a shard resident on demand — that is the point of
/// splitting, and it needs the `runtime-fonts` feature to have somewhere to register the bytes.
#[cfg_attr(
    docsrs,
    doc(cfg(any(
        feature = "fonts-cjk-shard-latin",
        feature = "fonts-cjk-shard-symbols",
        feature = "fonts-cjk-shard-kana",
        feature = "fonts-cjk-shard-fullwidth",
        feature = "fonts-cjk-shard-han"
    )))
)]
pub mod cjk_shards;

// Loading a shard on demand, into the runtime face list. Needs both a shard to load and the
// registry to load it into; without `runtime-fonts` a shard would have to be compiled into
// `active_faces`, which is the behaviour the shards exist to avoid.
#[cfg(all(
    feature = "runtime-fonts",
    any(
        feature = "fonts-cjk-shard-latin",
        feature = "fonts-cjk-shard-symbols",
        feature = "fonts-cjk-shard-kana",
        feature = "fonts-cjk-shard-fullwidth",
        feature = "fonts-cjk-shard-han"
    )
))]
#[cfg_attr(
    docsrs,
    doc(cfg(all(
        feature = "runtime-fonts",
        any(
            feature = "fonts-cjk-shard-latin",
            feature = "fonts-cjk-shard-symbols",
            feature = "fonts-cjk-shard-kana",
            feature = "fonts-cjk-shard-fullwidth",
            feature = "fonts-cjk-shard-han"
        )
    )))
)]
pub mod cjk_shard_loader;

#[cfg(feature = "fonts-emoji-color")]
mod emoji;
#[cfg(feature = "fonts-emoji-color")]
pub use emoji::FONT as EMOJI;

#[cfg(feature = "fonts-vector-latin")]
const LATIN_FACE: FaceBytes = FaceBytes { name: "Open Sans", bytes: LATIN };

#[cfg(feature = "fonts-complex")]
const ARABIC_FACE: FaceBytes = FaceBytes { name: "Noto Naskh Arabic", bytes: ARABIC };

#[cfg(feature = "fonts-cjk")]
const CJK_FACE: FaceBytes = FaceBytes { name: "Noto Sans SC", bytes: CJK };

#[cfg(feature = "fonts-emoji-color")]
const EMOJI_FACE: ColorFaceBytes = ColorFaceBytes { name: "Noto Color Emoji", bytes: EMOJI };

/// The colour bitmap faces this build carries, in preference order.
///
/// Separate from [`active_faces`] because a colour face answers a different question: it is not a
/// fallback for text, it is the *only* source for a character outside the text faces, and its ink is
/// colour rather than coverage. Keeping the two lists apart is what lets the glyph stack place it
/// correctly — see `render::text::glyph_source::active_stack`.
#[cfg(feature = "fonts-emoji-color")]
pub fn active_color_faces() -> &'static [ColorFaceBytes] {
    static SLOTS: [ColorFaceBytes; 1] = [EMOJI_FACE];
    &SLOTS
}

/// Whether `face`'s glyph table has `ch`.
///
/// One spelling of the coverage question, shared by [`face_for_char`] and [`outline_face_for`], so
/// the two cannot drift into disagreeing about what a face contains.
///
/// # Why this parses on every call, and why that is about to change
///
/// `ttf_parser::Face::parse` walks the table directory — a few dozen reads — and this function is on
/// the **per-glyph, per-frame** path (`paint_active` → `VectorSource::paint` → `face_for`). A label
/// of *n* Latin characters re-parses the same face *n* times a frame, every frame, for a table that
/// never changes. See [`face_for_char`], which layers a cache in front of this.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
pub(crate) fn face_covers(face: &FaceBytes, ch: char) -> bool {
    ttf_parser::Face::parse(face.bytes, 0).ok().and_then(|parsed| parsed.glyph_index(ch)).is_some()
}

/// How many entries the coverage cache holds.
///
/// A direct-mapped cache keyed by codepoint, so the entry for `ch` is `ch as usize %
/// COVERAGE_CACHE_SLOTS`. 64 slots is chosen against the shape of the workload rather than a
/// benchmark: a line of running text draws each character once, so the cache pays off on the
/// *repeat* case — the same label across frames, a repeated character in a run, a control's caption
/// drawn then measured. 64 codepoints covers a caption and then some; a larger table only reduces
/// collisions between characters that are not drawn together anyway.
///
/// Collisions are harmless: a miss simply falls through to the face walk, so the cache is an
/// accelerator, never an authority.
const COVERAGE_CACHE_SLOTS: usize = 64;

/// One cache slot: the codepoint, and the face (or "none") that answered for it.
///
/// The answer is stored as a `CacheAnswer` rather than an index into a candidate list, because the
/// candidate list changes when a host registers a face and an index would then point at the wrong
/// entry. Storing the two words of the `FaceBytes` makes a hit self-contained: it needs no lock on
/// the registry and no generation check, since it *is* the resolved answer.
#[derive(Clone, Copy)]
struct CoverageSlot {
    /// The codepoint this slot was filled for, or `NONE` when the slot is empty.
    codepoint: u32,
    /// What answered. `Some` is a face whose glyph table had the character; `None` was cached as
    /// well, so a character no face covers costs the walk once rather than once per frame.
    answer: Option<FaceBytes>,
}

impl CoverageSlot {
    /// A codepoint no real character has, so an empty slot can never match.
    const EMPTY_CODEPOINT: u32 = u32::MAX;
    const EMPTY: Self = Self { codepoint: Self::EMPTY_CODEPOINT, answer: None };
}

/// The process-wide coverage cache.
///
/// # Why a mutex, and why that does not contradict the glyph-path constraints
///
/// The crate's constraints forbid shared mutable state on the glyph path. This is a deliberate,
/// narrow exception: the state is a cache whose *only* correctness requirement is that a stale entry
/// is never wrong — and a slot stores the resolved answer, so a stale entry cannot be wrong. A
/// mutex (not a lock-free table) is what makes the lookup-and-fill atomic, so no torn slot is
/// reachable. The lock is taken per character and never held across rasterisation.
///
/// # Why the cache does not need invalidating when a face is registered
///
/// A slot holds the *answer*, not an index that a registration would shift. Registering a face
/// changes what a **later** lookup returns, and the next lookup for a codepoint whose slot holds the
/// old answer would still return the old face. That is why [`invalidate_face_cache`] exists: it
/// bumps a counter that every slot's tag is compared against, so a registration is visible to the
/// cache without the registry lock being taken on every glyph.
static COVERAGE_CACHE: crate::compat::Mutex<[CoverageSlot; COVERAGE_CACHE_SLOTS]> =
    crate::compat::Mutex::new([CoverageSlot::EMPTY; COVERAGE_CACHE_SLOTS]);

/// A parsed face, held for reuse across glyphs and frames.
///
/// # Why the parse has to be cached separately from the coverage answer
///
/// [`face_for_char`] caches *which* face covers a character, but every caller then parsed those
/// bytes again: `ttf_parser::Face::parse` walks the table directory, and it ran once per glyph, per
/// frame. Measured on `demo/control`, a frame of 36 controls took **25.6ms to render** and under
/// 1ms to blit, which is the parse and nothing else — a repaint that should be sub-millisecond work
/// was 25x the cost of presenting it.
///
/// The parsed value is borrowed from the face's own bytes, and a compiled face is `&'static`, so a
/// parsed `ttf_parser::Face<'static>` can be held here without copying the font. That is what makes
/// the reuse a borrow rather than an allocation.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
struct ParsedFaceSlot {
    /// Identity of the face the parse belongs to.
    ///
    /// A face is identified by the **address of its bytes**, not by its name: two builds can ship
    /// different bytes under one family, and the address is what proves a parse describes the font
    /// it is being used for. `FaceBytes::bytes` is `&'static [u8]`, so the address is stable.
    bytes: *const u8,
    /// The parsed face, or `None` when the bytes are not a face this crate can read. Caching the
    /// *failure* matters as much as caching the success: a miss would otherwise re-parse on every
    /// glyph of every frame for a character no face covers.
    face: Option<ttf_parser::Face<'static>>,
}

// SAFETY: `ttf_parser::Face` borrows its bytes and holds only offsets into them; it performs no
// interior mutation and is `Send + Sync`, so sharing one parsed face between threads is sound. The
// pointer identifies a `&'static [u8]`, which outlives the process's font registry.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
unsafe impl Send for ParsedFaceSlot {}
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
unsafe impl Sync for ParsedFaceSlot {}

/// How many parsed faces are held at once.
///
/// A build has at most four compiled faces plus a handful of host-registered ones, and a frame
/// practically draws from one or two, so four slots cover the working set with room for the
/// Latin+CJK mix a mixed line produces. A miss re-parses and replaces the least useful slot, which
/// costs one parse rather than being wrong.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
const PARSED_FACE_SLOTS: usize = 4;

/// The process-wide parsed-face cache, most-recently-used first.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
static PARSED_FACES: crate::compat::Mutex<[Option<ParsedFaceSlot>; PARSED_FACE_SLOTS]> =
    crate::compat::Mutex::new([None, None, None, None]);

/// Runs `f` against `bytes` parsed as a face, parsing it only when the cache does not hold it.
///
/// # Why a closure rather than returning the face
///
/// A parsed face is borrowed from a `Mutex`-guarded slot, so handing it out would either borrow the
/// lock for the caller's whole rasterisation or require an owned copy of the font. The closure keeps
/// the lock scoped to the call: the caller does its work and the guard is released on return, which
/// is the same discipline [`face_for_char`] already uses for its own cache.
///
/// Returns `None` when the bytes are not a readable face, matching the un-cached
/// `ttf_parser::Face::parse(...).ok()?` this replaces.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
pub fn with_parsed_face<R>(
    face: FaceBytes,
    f: impl FnOnce(&ttf_parser::Face<'_>) -> R,
) -> Option<R> {
    let key = face.bytes.as_ptr();
    let mut cache = crate::compat::lock(&PARSED_FACES);

    // A hit is self-contained: the slot holds the parse for exactly these bytes.
    if let Some(index) = cache.iter().position(|slot| slot.as_ref().is_some_and(|s| s.bytes == key))
    {
        let slot = cache[index].as_ref()?;
        return slot.face.as_ref().map(f);
    }

    let parsed = ttf_parser::Face::parse(face.bytes, 0).ok();
    // A miss with an unreadable face is cached too, so a character only the tofu path can draw does
    // not re-parse the candidate faces on every glyph of every frame.
    let answer = parsed.as_ref().map(f);

    // Most-recently-used first: shift the residents down and take slot 0. `take` moves the value
    // out without requiring `Clone` (a parsed face owns no font data, but it is still not `Copy`).
    // A frame draws from one or two faces, so the head of the list is what lookups want.
    for index in (1..PARSED_FACE_SLOTS).rev() {
        cache[index] = cache[index - 1].take();
    }
    cache[0] = Some(ParsedFaceSlot { bytes: key, face: parsed });
    answer
}

/// Drops every cached parse, so the next `with_parsed_face` reads the font again.
///
/// Called from [`invalidate_face_cache`] so a host that registers or clears faces does not keep
/// reading a parse taken before the change.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
pub fn clear_parsed_faces() {
    let mut cache = crate::compat::lock(&PARSED_FACES);
    *cache = [None, None, None, None];
}

/// A face parsed by the **shaper**, held for reuse across measurement and shaping passes.
///
/// # Why this is a second cache and not the first one
///
/// `rustybuzz::Face` and `ttf_parser::Face` are different parses of the same bytes: the shaper's
/// builds the OpenType layout tables (GSUB/GPOS) that `ttf_parser` does not, which is what makes it
/// expensive enough to matter. The rasteriser needs one and the shaper needs the other, so caching
/// either alone leaves the other on the per-call path.
///
/// This one is what a text control actually pays: `text_line` measures a probe character, then
/// `draw_text` shapes the real string, so a single label shaped the font **twice per frame** — and
/// every control in the window does the same. Measured, one `measure_text("M")` cost ~18us.
#[cfg(feature = "text-shaping")]
struct ShaperFaceSlot {
    /// Identity of the face, by the address of its bytes (see [`ParsedFaceSlot::bytes`]).
    bytes: *const u8,
    /// The parsed face, or `None` when the shaper cannot read these bytes.
    face: Option<rustybuzz::Face<'static>>,
}

// SAFETY: `rustybuzz::Face` borrows its bytes and exposes no interior mutation through a shared
// reference, so it is safe to share between threads. The pointer identifies a `&'static [u8]`.
#[cfg(feature = "text-shaping")]
unsafe impl Send for ShaperFaceSlot {}
#[cfg(feature = "text-shaping")]
unsafe impl Sync for ShaperFaceSlot {}

/// The process-wide shaper-face cache, most-recently-used first.
#[cfg(feature = "text-shaping")]
static SHAPER_FACES: crate::compat::Mutex<[Option<ShaperFaceSlot>; PARSED_FACE_SLOTS]> =
    crate::compat::Mutex::new([None, None, None, None]);

/// Runs `f` against `bytes` parsed by the shaper, parsing only when the cache does not hold it.
///
/// See [`with_parsed_face`] for why this takes a closure: the parsed face is borrowed from a
/// `Mutex`-guarded slot, so the lock is scoped to the call rather than to the caller's work.
#[cfg(feature = "text-shaping")]
pub fn with_shaper_face<R>(
    face: FaceBytes,
    f: impl FnOnce(&rustybuzz::Face<'_>) -> R,
) -> Option<R> {
    let key = face.bytes.as_ptr();
    let mut cache = crate::compat::lock(&SHAPER_FACES);

    if let Some(index) = cache.iter().position(|slot| slot.as_ref().is_some_and(|s| s.bytes == key))
    {
        let slot = cache[index].as_ref()?;
        return slot.face.as_ref().map(f);
    }

    let parsed = rustybuzz::Face::from_slice(face.bytes, 0);
    let answer = parsed.as_ref().map(f);
    for index in (1..PARSED_FACE_SLOTS).rev() {
        cache[index] = cache[index - 1].take();
    }
    cache[0] = Some(ShaperFaceSlot { bytes: key, face: parsed });
    answer
}

/// Drops every cached shaper parse; the shaper counterpart of [`clear_parsed_faces`].
#[cfg(feature = "text-shaping")]
pub fn clear_shaper_faces() {
    let mut cache = crate::compat::lock(&SHAPER_FACES);
    *cache = [None, None, None, None];
}

/// Bumped by every change to the face list, so entries from before it are ignored.
///
/// The registry can be mutated by a host calling `register_face`/`clear_faces` at any time. A slot
/// filled before such a call holds the pre-change answer; comparing the slot's tag against this
/// counter makes that entry a miss rather than a stale hit.
static FACE_GENERATION: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

/// The generation tag of each cache slot, in a parallel array sharing the slot index.
static SLOT_GENERATIONS: crate::compat::Mutex<[u32; COVERAGE_CACHE_SLOTS]> =
    crate::compat::Mutex::new([0u32; COVERAGE_CACHE_SLOTS]);

/// The face that covers `ch`, consulting **host-registered faces first**.
///
/// # Why this exists rather than every caller walking `active_faces`
///
/// With the `runtime-fonts` feature on, a host may register its own face
/// ([`crate::render::text::register_face`]). That face must be consulted before the compiled
/// ones — a host that supplied a font did so because it wants *that* font — so the lookup cannot
/// simply iterate the compiled list. Every caller that needs "which face draws this character"
/// (`VectorSource`, the SVG backend's `outline_by_coverage`, the shaper's coverage probe) routes
/// through here, which is what keeps them agreeing (the defect `outline_face_for` documents).
///
/// # The cache, and the work it removes
///
/// This function is on the **per-glyph, per-frame** path (`paint_active` → `VectorSource::paint` →
/// `face_for`). Un-cached, each call runs `ttf_parser::Face::parse` over every candidate face before
/// finding the one that covers the character — for a Latin label that is one parse per glyph; for a
/// CJK glyph it is a parse of Latin *and* CJK. Nothing about the answer changes between frames on a
/// build with no runtime faces, so it is work the process repeats purely because nothing remembered
/// it. The cache ([`COVERAGE_CACHE_SLOTS`]) remembers it per codepoint, `tofu` included.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
pub fn face_for_char(ch: char) -> Option<FaceBytes> {
    let codepoint = ch as u32;
    let slot = codepoint as usize % COVERAGE_CACHE_SLOTS;
    let generation = FACE_GENERATION.load(core::sync::atomic::Ordering::Acquire);

    // A hit is self-contained: the slot holds the resolved answer, so it needs neither the registry
    // nor the candidate walk. The tag check is what makes a registration visible.
    {
        let cache = crate::compat::lock(&COVERAGE_CACHE);
        let entry = cache[slot];
        if entry.codepoint == codepoint && slot_generation(slot) == generation {
            return entry.answer;
        }
    }

    let answer = resolve_face(ch);
    {
        let mut cache = crate::compat::lock(&COVERAGE_CACHE);
        cache[slot] = CoverageSlot { codepoint, answer };
    }
    set_slot_generation(slot, generation);
    answer
}

/// [`face_for_char`] **without** the cache — the control a benchmark measures the cache against.
///
/// # Why this is public and hidden
///
/// The cache's whole claim is "a repeated character is cheaper than the first". Measuring that
/// requires both sides: the cached call and the un-cached one. The un-cached one is otherwise
/// unreachable from a benchmark (an integration target is an external crate, so `pub(crate)` does
/// not cross), so it is exposed with `#[doc(hidden)]` — present for measurement and for a test that
/// asserts the two agree, but not part of the documented surface.
///
/// # The contract a measurement depends on
///
/// This function returns **exactly** what a cold [`face_for_char`] returns: same precedence
/// (runtime faces before compiled ones), same coverage rule. `tests::the_cached_and_uncached_lookups_agree`
/// asserts that equality, so a divergence is a test failure rather than a silently invalid benchmark.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
#[doc(hidden)]
pub fn face_for_char_uncached(ch: char) -> Option<FaceBytes> {
    resolve_face(ch)
}

/// The un-cached lookup: runtime faces first, then the compiled ones.
fn resolve_face(ch: char) -> Option<FaceBytes> {
    #[cfg(feature = "runtime-fonts")]
    {
        let (runtime, len) = crate::render::text::runtime_fonts::active_runtime_faces();
        for face in &runtime[..len] {
            if face_covers(face, ch) {
                return Some(*face);
            }
        }
    }
    active_faces().iter().copied().find(|face| face_covers(face, ch))
}

fn slot_generation(slot: usize) -> u32 {
    crate::compat::lock(&SLOT_GENERATIONS)[slot]
}

fn set_slot_generation(slot: usize, generation: u32) {
    crate::compat::lock(&SLOT_GENERATIONS)[slot] = generation;
}

/// Invalidate the coverage cache after the face list changes.
///
/// Called by `runtime_fonts` on every registration and clear, because those are the two operations
/// that change which face answers a character. A generation bump makes every pre-change entry a miss
/// without walking (and holding) 64 slots.
///
/// Gated on `runtime-fonts` because the registry is the only thing that can change the face list at
/// runtime: without it the cache's inputs are fixed for the life of the process, so there is nothing
/// to invalidate and no caller.
#[cfg(feature = "runtime-fonts")]
pub(crate) fn invalidate_face_cache() {
    FACE_GENERATION.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
    // Both parse caches are keyed by byte address rather than by generation, so a registered face
    // would otherwise keep reading a parse the registry no longer stands behind. Dropping the
    // residents costs one parse per face on the next frame and cannot serve stale bytes.
    clear_parsed_faces();
    clear_shaper_faces();
}

/// The vector faces this build carries, in preference order.
///
/// A face is chosen by *coverage* (the shaper asks each whether it has a glyph for the text's
/// first strong character), so order matters only for a character two faces both have: the
/// script-specific face is listed first, exactly as in the bitmap stack, for the same reason.
///
/// Order here is **CJK, then Arabic, then Latin**. CJK goes first because it is the widest script
/// with a face here and the one a Latin face must never answer for; Arabic before Latin because
/// Arabic's joining forms are the reason `fonts-complex` exists, and a character both faces have
/// (ASCII) should measure through the script-matching face when the caller asked for that script.
///
/// An empty list is a valid answer, and the common one: a build that enables `text-shaping` but no
/// generated face ships no data here and lets the host supply its own. That is why the non-data
/// case is a zero-length array rather than an absent function — the shaper's lookup code is the
/// same either way, so there is no second code path to keep in step.
///
/// This is the **compiled** list; a face registered at runtime via
/// [`crate::render::text::register_face`] is not in it. Use [`face_for_char`] when the question is
/// "which face draws this character", which is almost always.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    feature = "fonts-cjk"
))]
pub fn active_faces() -> &'static [FaceBytes] {
    // Enumerated rather than built by a loop: the list is a `&'static [FaceBytes]`, so it has to be
    // a `static` per combination, and enumerating them is what lets the compiler check that every
    // arm's length matches its contents. The eight combinations of the three data features follow.
    #[cfg(all(feature = "fonts-cjk", feature = "fonts-complex", feature = "fonts-vector-latin"))]
    static SLOTS: [FaceBytes; 3] = [CJK_FACE, ARABIC_FACE, LATIN_FACE];
    #[cfg(all(
        feature = "fonts-cjk",
        feature = "fonts-complex",
        not(feature = "fonts-vector-latin")
    ))]
    static SLOTS: [FaceBytes; 2] = [CJK_FACE, ARABIC_FACE];
    #[cfg(all(
        feature = "fonts-cjk",
        not(feature = "fonts-complex"),
        feature = "fonts-vector-latin"
    ))]
    static SLOTS: [FaceBytes; 2] = [CJK_FACE, LATIN_FACE];
    #[cfg(all(
        feature = "fonts-cjk",
        not(feature = "fonts-complex"),
        not(feature = "fonts-vector-latin")
    ))]
    static SLOTS: [FaceBytes; 1] = [CJK_FACE];
    #[cfg(all(
        not(feature = "fonts-cjk"),
        feature = "fonts-complex",
        feature = "fonts-vector-latin"
    ))]
    static SLOTS: [FaceBytes; 2] = [ARABIC_FACE, LATIN_FACE];
    #[cfg(all(
        not(feature = "fonts-cjk"),
        feature = "fonts-complex",
        not(feature = "fonts-vector-latin")
    ))]
    static SLOTS: [FaceBytes; 1] = [ARABIC_FACE];
    #[cfg(all(
        not(feature = "fonts-cjk"),
        not(feature = "fonts-complex"),
        feature = "fonts-vector-latin"
    ))]
    static SLOTS: [FaceBytes; 1] = [LATIN_FACE];
    #[cfg(not(any(
        feature = "fonts-cjk",
        feature = "fonts-complex",
        feature = "fonts-vector-latin"
    )))]
    static SLOTS: [FaceBytes; 0] = [];

    &SLOTS
}

/// The outline face that covers `ch`, or `None` when this build ships none.
///
/// # Why this exists as one function rather than two lookups
///
/// Two callers need "which face draws this character": [`VectorSource`](crate::render::text::VectorSource),
/// which rasterises it for the pixels, and the SVG backend, which emits its outline as geometry. They
/// must answer the same or the snapshot stops being a picture of the control.
///
/// They did **not** agree. `VectorSource` asks *by coverage* — the first face whose glyph table has
/// `ch` — while the SVG backend asked *by family name*, because `text::outline` takes the `Font`'s
/// family and looks it up. Every theme in this crate names `"Arial"` (and `"Courier New"`, and
/// `"sans-serif"`), while the faces it ships are called `"Open Sans"` and `"Noto Sans SC"`. So the
/// family lookup answered `None` for every glyph of every control:
///
/// ```text
/// $ cargo test --features fonts-vector-latin -- --nocapture
/// family="Arial"       -> outline_selected=false
/// family="Open Sans"   -> outline_selected=true
/// family="sans-serif"  -> outline_selected=false
/// ```
///
/// The SVG backend therefore took its 1-bit fallback for all 377 snapshots — each Latin glyph drawn
/// as ~130 one-pixel rectangles from the 8x8 bitmap, which is the blocky text the snapshots showed —
/// while the **runtime drew real outlines** (`paint_active('H')` reports `source=Open Sans
/// ink=Coverage` on the same build). A backend that disagrees with the rasteriser about which face is
/// in play is the one thing this backend's own docs say it exists to prevent, so the answer is a
/// shared lookup rather than a second rule.
///
/// # What this does *not* change
///
/// It is not a fallback for a mis-named font, and it does not re-lay-out anything. Measurement still
/// goes through [`crate::render::text::shape_line`], which still honours the family — so a caller who
/// names a face this build ships gets that face's advances, and a caller who names one it does not
/// gets the estimate model, exactly as `shaping::face_for_family` documents. What is now consistent
/// is only the *in* question: whichever face the rasteriser would use for a character is the face
/// whose outline the snapshot emits.
#[cfg(any(
    feature = "text-shaping",
    feature = "fonts-vector-latin",
    feature = "fonts-complex",
    cjk_outline_face
))]
pub fn outline_face_for(ch: char) -> Option<FaceBytes> {
    // One lookup, shared with `face_for_char`: the coverage question has exactly one spelling so the
    // rasteriser and the vector backend cannot answer it differently. Host-registered faces
    // (`runtime-fonts`) are consulted first there, which is also what this backend must do — an
    // outline emitted for a character must be the outline the pixels would show.
    face_for_char(ch)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cache is an accelerator, not an authority: a hit must equal the un-cached answer.
    ///
    /// # Why this is asserted rather than assumed
    ///
    /// The benchmark in `benches/font_bench.rs` measures `face_for_char` against
    /// `face_for_char_uncached`; that comparison is only meaningful if the two are the *same
    /// function* modulo the cache. A cache that returned a different face — from a stale index, a
    /// generation bug, or a bad slot mapping — would make the benchmark compare apples to oranges
    /// while every timing looked plausible. This is the assertion that keeps the control honest.
    #[cfg(any(
        feature = "text-shaping",
        feature = "fonts-vector-latin",
        feature = "fonts-complex",
        cjk_outline_face
    ))]
    #[test]
    fn the_cached_and_uncached_lookups_agree() {
        // A spread that exercises: a covered ASCII letter, an uncovered private-use codepoint
        // (cached as "no face"), whitespace, and — when the build carries them — CJK and an
        // emoji, so both the compiled-list and the shape of the answer are covered.
        let probes = ['A', 'z', '\u{0}', ' ', '\u{E000}', '\u{4E2D}', '\u{3042}', '\u{1F600}'];
        for ch in probes {
            let cached = face_for_char(ch);
            let uncached = face_for_char_uncached(ch);
            assert_eq!(
                cached.map(|face| face.name),
                uncached.map(|face| face.name),
                "cache and un-cached lookup disagree for U+{:04X} ({ch:?})",
                ch as u32
            );
        }
    }

    /// A repeated lookup returns the first answer, and a build with no face answers `None`.
    ///
    /// The second half is the honest-absence contract: `face_for_char` on a build with an empty
    /// compiled list and no registered face must say "no face", not fabricate one.
    #[cfg(any(
        feature = "text-shaping",
        feature = "fonts-vector-latin",
        feature = "fonts-complex",
        cjk_outline_face
    ))]
    #[test]
    fn a_repeated_lookup_is_stable() {
        let first = face_for_char('A');
        for _ in 0..100 {
            assert_eq!(
                face_for_char('A').map(|face| face.name),
                first.map(|face| face.name),
                "the cached answer must not change across repeated lookups"
            );
        }
    }
}
