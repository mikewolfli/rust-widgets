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
