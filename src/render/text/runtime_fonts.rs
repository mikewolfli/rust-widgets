// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Runtime face registration: a host's own font bytes, added to the active face list.
//!
//! # The gap this closes
//!
//! Until this module existed every face was chosen at **compile time** — `include_bytes!` in
//! `font_assets::<id>.rs`, selected by a `fonts-*` feature. `FaceBytes`' own docs already stated the
//! intended escape hatch ("a caller can turn on `text-shaping` to supply their own face at runtime,
//! with no generated data at all"), but no entry point existed to actually hand one in. A host with
//! a system font (PingFang, Microsoft YaHei, a Noto the crate does not ship) had no way to use it
//! without recompiling the library.
//!
//! This module is that entry point: [`register_face`] installs a host-supplied face, and
//! [`active_faces`](crate::render::text::font_assets::active_faces) merges it **ahead of** the
//! compiled faces so a host face always wins for a character it covers.
//!
//! # Why the bytes are `&'static [u8]`
//!
//! The face list is `&'static [FaceBytes]` and is read by the shaper and the rasteriser on every
//! glyph, from threads that hold no lock. Accepting an owned `Vec<u8>` would force either a lock on
//! that path or a leak, and the crate's three constraints (no locks on the glyph path, no resident
//! tables, no allocation per glyph) forbid both. A `&'static [u8]` is what a host gets for free from
//! `include_bytes!`, a `Box::leak`, or `Box::leak(Vec::into_boxed_slice())` — the leak is explicit
//! and paid once, and the bytes are then reachable without synchronisation.
//!
//! # Registration ordering
//!
//! Faces are stored in a fixed-capacity array ([`MAX_RUNTIME_FACES`]) and appended in registration
//! order: the most recently registered face is consulted **first**. That matches the intuition a
//! caller has when they say "use *this* font" — the last registration is the caller's current
//! choice, and an earlier one stays reachable as a fallback rather than being silently ignored.
//!
//! # When `runtime-fonts` is off
//!
//! The whole module is gated on the feature. Without it [`active_runtime_faces`] does not exist and
//! `active_faces` is the compiled list, exactly as before — so a build that does not want the
//! registry gains neither the code nor the static.

#![cfg(feature = "runtime-fonts")]

use crate::compat::Mutex;
use crate::render::text::font_assets::FaceBytes;

/// How many host faces may be registered at once.
///
/// A fixed capacity rather than a growable vector: the array is read while holding the registry lock
/// and copied out, and a `Vec` reachable from the registry would let a caller registered concurrent
/// with a read tear the list. Four is a UI's worth of families (a text face, a mono face, and a
/// script face or two) without making the per-lookup copy large — the copy is four `Copy` structs of
/// two words each.
pub const MAX_RUNTIME_FACES: usize = 4;

/// The registry: registered faces and how many slots are occupied.
///
/// `faces` is `[FaceBytes; N]` behind a `Mutex` because registration happens from a host's setup
/// code (any thread) and reads happen per glyph (any thread). The lock is held only for the copy
/// out, never across a rasterisation.
struct Registry {
    faces: [Option<FaceBytes>; MAX_RUNTIME_FACES],
    len: usize,
}

impl Registry {
    /// An empty registry. `const` so it can initialise the static without a lazy wrapper.
    const fn new() -> Self {
        Self { faces: [None; MAX_RUNTIME_FACES], len: 0 }
    }
}

static REGISTRY: Mutex<Registry> = Mutex::new(Registry::new());

/// Register `face` so it is consulted before every compiled face.
///
/// Returns `true` when the face was accepted, `false` when the registry is full or the bytes are not
/// a font `ttf-parser` can read. A `false` is a refusal, not a silent drop: a caller that registered
/// a face and got `false` must not believe its font is in use.
///
/// # Why the bytes are validated here
///
/// A face that cannot be parsed would sit in the list and be `None`-ed out on every lookup — the
/// list would grow, the lock would be taken, and nothing would render with it. Rejecting it at
/// registration makes the failure land where the caller can report it, rather than as glyphs that
/// silently never appear.
///
/// # Why re-registering the same name replaces rather than appends
///
/// A host that calls [`register_face`] twice with the same family name means "use this one instead",
/// not "keep both and prefer one arbitrarily". The replacement keeps the list free of two faces that
/// answer the same question, which is the condition `FaceBytes`' name-keyed lookup would otherwise
/// resolve depending on order alone.
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "runtime-fonts")] {
/// // The host's own bytes. `include_bytes!` gives a `&'static [u8]` for free; a file read at
/// // startup needs an explicit leak, which is paid once and then lock-free forever.
/// let bytes: &'static [u8] = Box::leak(std::fs::read("NotoSansSC-Regular.otf").unwrap().into_boxed_slice());
/// assert!(rust_widgets::render::text::register_face("Noto Sans SC", bytes));
/// # }
/// ```
pub fn register_face(name: &'static str, bytes: &'static [u8]) -> bool {
    if name.is_empty() || bytes.is_empty() {
        return false;
    }
    // A face with no glyph table is not a font this layer can use: prove it parses before it can
    // shadow a compiled face that does.
    if ttf_parser::Face::parse(bytes, 0).is_err() {
        return false;
    }
    let mut registry = crate::compat::lock(&REGISTRY);
    // Replace an existing face with the same family name, keeping its position.
    let len = registry.len;
    for slot in registry.faces[..len].iter_mut() {
        if let Some(existing) = slot {
            if existing.name.eq_ignore_ascii_case(name) {
                *slot = Some(FaceBytes { name, bytes });
                drop(registry);
                // A replacement changes which face answers a character, so the coverage cache's
                // entries for this family are now stale. Bump the generation; the next lookup misses.
                crate::render::text::font_assets::invalidate_face_cache();
                return true;
            }
        }
    }
    if len >= MAX_RUNTIME_FACES {
        return false;
    }
    registry.faces[len] = Some(FaceBytes { name, bytes });
    registry.len = len + 1;
    drop(registry);
    crate::render::text::font_assets::invalidate_face_cache();
    true
}

/// Forget every registered face. The compiled faces are unaffected.
///
/// # Why this exists
///
/// A test that registers a face would otherwise leak it into every later test in the process, and a
/// caller that switches fonts at runtime needs a way to fall back to the compiled set. Returning the
/// count makes the effect observable rather than something a test has to infer.
pub fn clear_faces() -> usize {
    let mut registry = crate::compat::lock(&REGISTRY);
    let removed = registry.len;
    registry.faces = [None; MAX_RUNTIME_FACES];
    registry.len = 0;
    drop(registry);
    // Every entry may now name a face that is no longer registered, so the whole cache is stale.
    crate::render::text::font_assets::invalidate_face_cache();
    removed
}

/// How many faces are currently registered.
pub fn registered_face_count() -> usize {
    crate::compat::lock(&REGISTRY).len
}

/// The registered faces, most-recently-registered **first**.
///
/// This is the order `active_faces` concatenates with, so a face registered last is consulted first.
/// The copy is at most [`MAX_RUNTIME_FACES`] two-word structs, taken under the lock and then released
/// before the caller does any per-glyph work.
pub(crate) fn active_runtime_faces() -> ([FaceBytes; MAX_RUNTIME_FACES], usize) {
    let registry = crate::compat::lock(&REGISTRY);
    let mut out: [FaceBytes; MAX_RUNTIME_FACES] =
        [FaceBytes { name: "", bytes: &[] }; MAX_RUNTIME_FACES];
    let mut len = 0;
    // Reverse: the last registration becomes index 0, which is the preference rule the module docs
    // state, and it is applied here rather than at the read site so every reader agrees.
    for slot in registry.faces[..registry.len].iter().rev().flatten() {
        out[len] = *slot;
        len += 1;
    }
    (out, len)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal face the parser accepts: Open Sans' compiled subset when the feature is on, or the
    /// embedded 8x8 face is not a font file at all. This test therefore needs a real face, and the
    /// crate always ships one on the CJK vector path — but not necessarily on a `runtime-fonts`-only
    /// build. Filling the registry with *parsable* bytes is what the assertions turn on, so the bytes
    /// come from a face this build is guaranteed to have only when one exists; otherwise the test
    /// exercises the refusal path, which is itself a contract.
    fn any_parsable_face() -> Option<&'static [u8]> {
        // `active_faces` here is the compiled list (this module is not consulted by it), so a
        // compiled face is exactly the "some other face" these tests need.
        crate::render::text::font_assets::active_faces().first().map(|face| face.bytes)
    }

    /// The registry is process-global, so tests that touch it must not interleave. A single test
    /// function owning the whole sequence is the lock-free way to guarantee that.
    #[test]
    fn registration_refuses_junk_and_honours_the_capacity_and_the_order() {
        // Start from a known state whatever the process did before this test.
        clear_faces();
        assert_eq!(registered_face_count(), 0);
        assert_eq!(active_runtime_faces().1, 0);

        // Junk is refused rather than stored: parsing is the admission test.
        assert!(!register_face("junk", b"not a font"));
        assert!(!register_face("empty", b""));
        assert!(!register_face("", b"1234"));
        assert_eq!(registry_len_for_test(), 0, "a refused face must not occupy a slot");

        let Some(bytes) = any_parsable_face() else {
            // No compiled face on this build: the refusal path is all that can be checked here, and
            // it was. A `runtime-fonts`-only build has nothing to register against.
            return;
        };

        assert!(register_face("First", bytes));
        assert!(register_face("Second", bytes));
        assert_eq!(registered_face_count(), 2);

        // Most-recently-registered first.
        let (faces, len) = active_runtime_faces();
        assert_eq!(len, 2);
        assert_eq!(faces[0].name, "Second");
        assert_eq!(faces[1].name, "First");

        // Re-registering a name replaces in place rather than appending.
        assert!(register_face("First", bytes));
        assert_eq!(registered_face_count(), 2, "a same-named registration replaces");
        let (faces, len) = active_runtime_faces();
        assert_eq!(len, 2);
        assert_eq!(
            faces[1].name, "First",
            "a replacement keeps the slot the original occupied, so the order is unchanged"
        );
        assert_eq!(faces[0].name, "Second");

        // Capacity: the two occupied slots plus two more reach `MAX_RUNTIME_FACES`, and the next is
        // refused rather than silently dropped.
        assert!(register_face("Third", bytes));
        assert!(register_face("Fourth", bytes));
        assert_eq!(registered_face_count(), MAX_RUNTIME_FACES);
        assert!(!register_face("Fifth", bytes), "a full registry must refuse, not evict");

        assert_eq!(clear_faces(), MAX_RUNTIME_FACES);
        assert_eq!(registered_face_count(), 0);
    }

    /// The occupied-slot count, reached through the public reader so the test does not need a second
    /// lock acquisition helper.
    fn registry_len_for_test() -> usize {
        registered_face_count()
    }
}
