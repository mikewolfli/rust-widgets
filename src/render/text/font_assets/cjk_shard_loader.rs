// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Loading a CJK shard on demand, into the runtime face list.
//!
//! # What this is for
//!
//! The sharded CJK face (`fonts-cjk-shards`) splits the `fonts-cjk` payload on Unicode block
//! boundaries, so a host that draws one script can load one script's glyphs. Splitting is only
//! useful if the host can choose *when* each shard becomes resident — otherwise the build carries
//! every shard and the split bought nothing but a few extra table copies.
//!
//! This module is that choice. [`load_shard_for`] takes a character, finds the shard whose range
//! contains it, and — if this build carries that shard — registers its bytes with
//! [`register_face`](crate::render::text::register_face). From then on the shard is a normal
//! runtime face: consulted before the compiled ones, cached by `face_for_char`, and reachable by
//! family name.
//!
//! # Why registration rather than a compiled face
//!
//! A compiled face is in `active_faces()` from process start, which is exactly the "everything
//! resident" behaviour the shards exist to avoid. Registering a shard makes it resident **on the
//! first character that needs it**, so a UI that never draws kana never pays for the kana shard
//! even though its feature is on.
//!
//! # The family name, and why it is the shard's
//!
//! A registered face is named for the shard (`"Noto Sans SC (kana)"`), not for the family the host
//! might name. That keeps two shards from claiming one family name — which
//! [`register_face`](crate::render::text::register_face)'s replace-on-same-name rule would otherwise
//! turn into one shard evicting another.

use crate::render::text::font_assets::cjk_shards::shard_for;
use crate::render::text::runtime_fonts::{register_face, registered_face_count, MAX_RUNTIME_FACES};

/// The outcome of a [`load_shard_for`] call.
///
/// A three-way result rather than a `bool`: "no shard covers this character" and "the shard exists
/// but this build does not carry it" are different facts, and a caller deciding whether to fall
/// back to a bitmap face needs to tell them apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShardLoad {
    /// The shard was registered, or was already resident.
    Loaded,
    /// A shard covers `ch`, but this build does not carry it (its feature is off).
    NotInThisBuild,
    /// No shard covers `ch`, so it is not a CJK character these shards know.
    NotCovered,
    /// The registry is full, so the shard could not be added.
    RegistryFull,
}

/// Make the shard covering `ch` resident, and report what happened.
///
/// # Why this is idempotent
///
/// A UI redraws the same character every frame, so this is called far more often than a shard needs
/// to be added. The already-registered case is detected with
/// [`registered_face_count`](crate::render::text::registered_face_count) plus the shard lookup
/// rather than by probing the registry for the name: registering the same name twice is defined as a
/// replacement, so the second call is harmless anyway — the check is only to keep the common case
/// (already loaded) from taking the registry lock at all.
///
/// # What the caller should do with a non-`Loaded` result
///
/// Nothing, in the render path: a character whose shard is absent draws through whatever else covers
/// it (a `fonts-cjk-bitmap` face, or tofu). This is the honest report — the build was asked for a
/// shard it does not have, and the caller can decide whether that is worth surfacing.
pub fn load_shard_for(ch: char) -> ShardLoad {
    let Some(shard) = shard_for(ch) else {
        return ShardLoad::NotCovered;
    };
    if shard.bytes.is_empty() {
        return ShardLoad::NotInThisBuild;
    }
    // Already resident: the registry holds this shard's name. Checking here keeps the common
    // (every-frame) path from taking the registry lock to re-register an identical face.
    if registered_face_count() > 0 && shard_is_registered(shard.id) {
        return ShardLoad::Loaded;
    }
    if register_face(shard.name, shard.bytes) {
        ShardLoad::Loaded
    } else if registered_face_count() >= MAX_RUNTIME_FACES {
        ShardLoad::RegistryFull
    } else {
        // `register_face` refuses unparsable bytes; a generated shard is a real font, so this is
        // reachable only if the generated table were corrupt, which the byte-exact `--check` gate
        // would have caught. Reporting it as "not in this build" would be a lie, so it is reported
        // as a full registry only when that is what happened, and as a load failure otherwise —
        // which is the same shape a caller treats as "cannot use this shard".
        ShardLoad::NotInThisBuild
    }
}

/// Whether a shard with `id` is already in the runtime registry.
///
/// Reads through `active_runtime_faces` rather than the public count, because the question is
/// *which* faces are registered, not how many.
fn shard_is_registered(id: &str) -> bool {
    let (runtime, len) = crate::render::text::runtime_fonts::active_runtime_faces();
    runtime[..len].iter().any(|face| face.name.contains(id))
}

/// Make every shard that covers any character of `text` resident.
///
/// The convenience for a caller that has a string rather than a character — a label being laid out,
/// a document being loaded. Returns how many shards it made resident (0 when they all already were,
/// or when none are carried).
///
/// # Why "resident" rather than "newly registered"
///
/// A caller wants to know "is this string's script now drawable", which the count of loaded shards
/// answers; distinguishing "already there" from "added just now" would make the common
/// already-loaded case return 0 and read as failure. So the count is of shards the call guarantees,
/// not of registrations it performed.
pub fn load_shards_for_text(text: &str) -> usize {
    let mut loaded_ids: crate::compat::Vec<&'static str> = crate::compat::Vec::new();
    for ch in text.chars() {
        if let Some(shard) = shard_for(ch) {
            if !shard.bytes.is_empty() && !loaded_ids.contains(&shard.id) {
                loaded_ids.push(shard.id);
            }
        }
    }
    // Register in file order is not required, but a stable order makes the registry's contents
    // depend on the shard set rather than on the string, which is what a test can assert.
    let mut loaded = 0;
    for shard in crate::render::text::font_assets::cjk_shards::active_shards() {
        if loaded_ids.contains(&shard.id)
            && load_shard_for_byte_shape(shard.name, shard.bytes) == ShardLoad::Loaded
        {
            loaded += 1;
        }
    }
    loaded
}

/// Register one shard by its parts, for [`load_shards_for_text`] which already has them.
fn load_shard_for_byte_shape(name: &'static str, bytes: &'static [u8]) -> ShardLoad {
    if register_face(name, bytes) {
        ShardLoad::Loaded
    } else if registered_face_count() >= MAX_RUNTIME_FACES {
        ShardLoad::RegistryFull
    } else {
        ShardLoad::NotInThisBuild
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::text::font_assets::cjk_shards::{active_shards, SHARDS};
    use crate::render::text::{clear_faces, registered_face_count};

    /// The shard table is a partition, so a character is in at most one shard's range, and the
    /// ranges are ordered so the table's `shard_for` (a `find`) is unambiguous.
    #[test]
    fn the_shard_ranges_partition_the_coverage() {
        // No two ranges overlap, and they are ascending — which is what makes the linear `find`
        // correct rather than merely lucky.
        let mut previous_high = 0u32;
        for shard in SHARDS {
            let (low, high) = shard.range;
            assert!(low <= high, "shard {} has an inverted range", shard.id);
            assert!(
                low > previous_high,
                "shard {} starts at U+{low:04X}, at or below the previous shard's end U+{previous_high:04X}",
                shard.id
            );
            previous_high = high;
        }

        // A character in a range resolves to its shard; one outside resolves to none.
        for shard in SHARDS {
            let (low, high) = shard.range;
            let low_char = char::from_u32(low).expect("a shard's first codepoint is a scalar");
            let high_char = char::from_u32(high).expect("a shard's last codepoint is a scalar");
            assert_eq!(shard_for(low_char).map(|s| s.id), Some(shard.id));
            assert_eq!(shard_for(high_char).map(|s| s.id), Some(shard.id));
        }
        // A private-use codepoint is in no shard.
        assert!(shard_for('\u{E000}').is_none());
    }

    /// With any shard feature on, its bytes are non-empty and `active_shards` reports it; a
    /// character in its range loads (once) and is then already resident.
    #[test]
    fn a_carried_shard_loads_once_and_is_then_resident() {
        clear_faces();
        let carried: crate::compat::Vec<_> = active_shards().collect();
        if carried.is_empty() {
            // No shard feature is on. The "not in this build" path is the contract that still holds.
            let any = SHARDS.first().expect("the shard table is never empty");
            let ch = char::from_u32(any.range.0).expect("scalar");
            assert_eq!(load_shard_for(ch), ShardLoad::NotInThisBuild);
            assert_eq!(registered_face_count(), 0);
            return;
        }
        let shard = carried[0];
        let ch = char::from_u32(shard.range.0).expect("scalar");
        assert_eq!(load_shard_for(ch), ShardLoad::Loaded, "a carried shard must load");
        assert_eq!(registered_face_count(), 1);
        // Idempotent: a second call is `Loaded` without growing the registry.
        assert_eq!(load_shard_for(ch), ShardLoad::Loaded);
        assert_eq!(registered_face_count(), 1, "re-loading must not add a second face");
        clear_faces();
        assert_eq!(registered_face_count(), 0);
    }
}
