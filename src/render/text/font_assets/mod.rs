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
#[derive(Clone, Copy)]
pub struct FaceBytes {
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

#[cfg(feature = "fonts-vector-latin")]
const LATIN_FACE: FaceBytes = FaceBytes { name: "Open Sans", bytes: LATIN };

#[cfg(feature = "fonts-complex")]
const ARABIC_FACE: FaceBytes = FaceBytes { name: "Noto Naskh Arabic", bytes: ARABIC };

/// The vector faces this build carries, in preference order.
///
/// A face is chosen by *coverage* (the shaper asks each whether it has a glyph for the text's
/// first strong character), so order matters only for a character two faces both have: the
/// script-specific face is listed first, exactly as in the bitmap stack, for the same reason.
pub fn active_faces() -> &'static [FaceBytes] {
    #[cfg(all(feature = "fonts-complex", feature = "fonts-vector-latin"))]
    static SLOTS: [FaceBytes; 2] = [ARABIC_FACE, LATIN_FACE];
    #[cfg(all(feature = "fonts-complex", not(feature = "fonts-vector-latin")))]
    static SLOTS: [FaceBytes; 1] = [ARABIC_FACE];
    #[cfg(all(feature = "fonts-vector-latin", not(feature = "fonts-complex")))]
    static SLOTS: [FaceBytes; 1] = [LATIN_FACE];
    #[cfg(not(any(feature = "fonts-vector-latin", feature = "fonts-complex")))]
    static SLOTS: [FaceBytes; 0] = [];

    &SLOTS
}
