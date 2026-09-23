// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Glyph sources — where a character's pixels come from.
//!
//! # Why this is a trait and not one function
//!
//! The crate used to answer "what does this character look like?" in exactly one place: an 8x8
//! bitmap table covering `U+0000`–`U+007F`. That is a *source*, not *the* answer — a CJK
//! character needs a 16x16 cell, a future vector face needs outlines, and a test wants a stub.
//! Naming the source as a trait lets each be added without touching the two renderers, and lets
//! the choice be data (a stack) rather than a code path.
//!
//! # The two axes this keeps apart
//!
//! **Having a glyph** and **deciding which font supplies it** are different questions, and the
//! crate conflates nothing by splitting them:
//!
//! * [`GlyphSource`] answers "do you have this character, and what are its pixels?".
//! * [`FontStack`] answers "among the sources I have, which one wins?" — first hit, in order.
//!
//! A source that does not cover a character returns `None`; it does **not** return a box. That
//! distinction is what makes a fallback chain possible at all: if the 8x8 face answered `Some`
//! with the tofu block for every character, no later source would ever be reached.
//!
//! # Cost
//!
//! Everything here is `Copy` or a `&'static` slice. No allocation, no `std`, no global mutable
//! state — a stack of sources is a fixed slice chosen by feature, so the default build resolves
//! a glyph through the same code it always did.

/// Whether a row's leftmost pixel is the most or least significant bit of its first byte.
///
/// The two bitmap faces in the crate disagree, and that is not a defect to paper over: the
/// `font8x8` table is indexed `1 << x` (leftmost = bit 0), while the Unifont `.hex` encoding
/// writes `0x8000` for a set leftmost pixel (leftmost = bit 15). Naming the difference here
/// means neither table has to be transformed at load time — which for the CJK table would mean
/// either an 85 KB const transform or a per-read reversal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitOrder {
    /// Bit `0` of a row is its leftmost pixel (the `font8x8` table).
    LsbFirst,
    /// The highest bit of a row is its leftmost pixel (the Unifont `.hex` table).
    MsbFirst,
}

/// One glyph's pixels, as packed rows.
///
/// Borrowed rather than owned: a source returns a view into its own table, so reading a glyph
/// costs a `Copy` of three words and never an allocation. That is also what keeps the CJK table
/// out of RAM (constraint: 7 000 CJK glyphs resident is infeasible on the small profiles) — the
/// data stays in the binary's read-only section and only the requested rows are touched.
#[derive(Debug, Clone, Copy)]
pub struct GlyphBitmap {
    /// Pixel width of the glyph's cell (`8`, `16`, …).
    pub width: u32,
    /// Pixel height of the glyph's cell.
    pub height: u32,
    /// How the row bytes are ordered.
    pub order: BitOrder,
    /// Packed rows: `ceil(width / 8)` bytes per row, row-major.
    rows: GlyphRows,
}

/// The storage behind a [`GlyphBitmap`]: inline for a fixed 8x8 face, borrowed for a large table.
///
/// The 8x8 face is returned **by value** by the `font8x8` crate, so a `&'static [u8]` cannot
/// express it; an inline array can. The CJK table is the opposite: 75 KB of static data that
/// must be borrowed, never copied. One enum covers both without an allocation.
#[derive(Debug, Clone, Copy)]
enum GlyphRows {
    /// Eight rows held inline (the 8x8 face).
    Inline8([u8; 8]),
    /// Rows borrowed from a generated static table.
    Borrowed(&'static [u8]),
}

impl GlyphBitmap {
    /// A bitmap for a glyph whose rows are held inline (an 8x8 cell, LSB-first).
    pub const fn inline8(rows: [u8; 8]) -> Self {
        Self { width: 8, height: 8, order: BitOrder::LsbFirst, rows: GlyphRows::Inline8(rows) }
    }

    /// A bitmap over a borrowed table, `width x height` pixels, MSB-first rows.
    ///
    /// `rows` must be `ceil(width / 8) * height` bytes; a short slice reads as blank rather than
    /// panicking, because a malformed generated table should render nothing, not take the process
    /// down inside a paint call.
    pub const fn borrowed(width: u32, height: u32, order: BitOrder, rows: &'static [u8]) -> Self {
        Self { width, height, order, rows: GlyphRows::Borrowed(rows) }
    }

    /// Whether the pixel at `(x, y)` is set.
    ///
    /// Out-of-bounds reads are `false`, for the same reason as a short row slice: a drawing
    /// routine is the wrong place to discover a bad table.
    pub fn bit(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let row_bytes = self.width.div_ceil(8) as usize;
        let index = y as usize * row_bytes + (x / 8) as usize;
        let byte = match &self.rows {
            GlyphRows::Inline8(rows) => match rows.get(index) {
                Some(byte) => *byte,
                None => return false,
            },
            GlyphRows::Borrowed(rows) => match rows.get(index) {
                Some(byte) => *byte,
                None => return false,
            },
        };
        let shift = match self.order {
            BitOrder::LsbFirst => x % 8,
            BitOrder::MsbFirst => 7 - (x % 8),
        };
        byte & (1u8 << shift) != 0
    }
}

/// A provider of glyph pixels.
///
/// Implementors are `Send + Sync` and live for the whole process, so a stack of them can be a
/// `&'static` slice with no locking.
pub trait GlyphSource: Send + Sync {
    /// This source's glyph for `ch`, or `None` when it does not cover the character.
    ///
    /// `None` is the honest answer for "not my character": it is what lets a stack fall through
    /// to the next source. A source that answered with a placeholder box for everything would
    /// make every later source unreachable.
    fn glyph(&self, ch: char) -> Option<GlyphBitmap>;

    /// A stable name, for diagnostics and for a test to assert which source answered.
    fn name(&self) -> &'static str;

    /// Whether this source covers `ch`. Defaults to asking [`Self::glyph`].
    fn covers(&self, ch: char) -> bool {
        self.glyph(ch).is_some()
    }
}

/// The crate's built-in 8x8 face: `U+0000`–`U+007F` in six styles.
///
/// This is the default build's only source, and it is deliberately last in every stack: it is
/// the face with the widest *Latin* coverage and the crate's historical behaviour, so it must
/// never be shadowed for a Latin character by a source that happens to also contain one.
pub struct Font8x8Source;

impl Font8x8Source {
    /// The one shared instance.
    pub const INSTANCE: Self = Self;
}

impl GlyphSource for Font8x8Source {
    fn glyph(&self, ch: char) -> Option<GlyphBitmap> {
        use font8x8::{UnicodeFonts, BASIC_FONTS};
        // The three lookups reproduce the crate's original resolution order exactly
        // (`ch`, then ASCII-uppercased, then ASCII-lowercased), so every snapshot taken before
        // this abstraction existed is unchanged.
        if let Some(rows) = BASIC_FONTS.get(ch) {
            return Some(GlyphBitmap::inline8(rows));
        }
        if let Some(rows) = BASIC_FONTS.get(ch.to_ascii_uppercase()) {
            return Some(GlyphBitmap::inline8(rows));
        }
        if let Some(rows) = BASIC_FONTS.get(ch.to_ascii_lowercase()) {
            return Some(GlyphBitmap::inline8(rows));
        }
        None
    }

    fn name(&self) -> &'static str {
        "font8x8"
    }
}

/// The block a character with no glyph is drawn as — a hollow box, "tofu".
///
/// # Why this is not a [`GlyphSource`]
///
/// Tofu is not a font that *covers* a character; it is what the renderer draws when **no** source
/// does. Modelling it as a source would make every stack end in a lie ("this face covers
/// everything"), and the fall-through that makes a stack work would stop. It belongs to the
/// resolver, not to the chain — see [`resolve`].
pub const TOFU: [u8; 8] = [
    0b11111111, 0b10000001, 0b10111101, 0b10100101, 0b10111101, 0b10000001, 0b11111111, 0b00000000,
];

/// An ordered list of sources; the first that covers `ch` answers.
///
/// # Order is the whole contract
///
/// Earlier entries win. A stack is therefore how a *preference* is expressed — "prefer the CJK
/// face for an ideograph, fall back to the 8x8 face for Latin" — without either source knowing
/// about the other. Because the composed list is a fixed slice, assembling one costs nothing and
/// needs no lock.
pub struct FontStack {
    sources: &'static [&'static dyn GlyphSource],
}

impl FontStack {
    /// Build a stack over `sources`, in preference order.
    pub const fn new(sources: &'static [&'static dyn GlyphSource]) -> Self {
        Self { sources }
    }

    /// The sources, in order.
    pub fn sources(&self) -> &'static [&'static dyn GlyphSource] {
        self.sources
    }

    /// The first source that covers `ch`, and its glyph.
    pub fn resolve(&self, ch: char) -> Option<(&'static dyn GlyphSource, GlyphBitmap)> {
        for source in self.sources {
            if let Some(glyph) = source.glyph(ch) {
                return Some((*source, glyph));
            }
        }
        None
    }

    /// [`Self::resolve`], falling back to [`TOFU`] so a caller always has something to draw.
    ///
    /// The second element reports **which source answered**, or `None` for the tofu fallback —
    /// which is what a test uses to assert the chain reached the right face.
    pub fn resolve_or_tofu(&self, ch: char) -> (GlyphBitmap, Option<&'static str>) {
        match self.resolve(ch) {
            Some((source, glyph)) => (glyph, Some(source.name())),
            None => (GlyphBitmap::inline8(TOFU), None),
        }
    }
}

/// The active font stack for this build.
///
/// # Why this is chosen by feature and not registered at runtime
///
/// Constraint: the **default build carries no font data**, and the small profiles must not grow a
/// registry to support data they do not have. Making the stack a `const` selected by feature
/// (rather than a process-global a caller mutates) means:
///
/// * the default build's stack is exactly `[font8x8]`, so its output is byte-identical;
/// * an opt-in data feature adds one entry, at compile time;
/// * there is no global mutable state to lock, order, or reset between tests.
///
/// A caller that wants a different stack (a test, a host with its own face) builds a
/// [`FontStack`] directly and passes it where it needs one.
pub fn active_stack() -> FontStack {
    // The stack with no opt-in font data: the crate's historical single face.
    #[cfg(not(feature = "fonts-cjk-bitmap"))]
    static BASE: [&dyn GlyphSource; 1] = [&Font8x8Source::INSTANCE];

    // The stack with the CJK bitmap face added.
    //
    // The CJK face comes **first** for the characters it covers, because it is the only face
    // that has them; the 8x8 face stays last so Latin resolution is untouched. Ordering it the
    // other way (8x8 first) would be harmless for Latin — the 8x8 face does not cover CJK — but
    // would leave the CJK face unreachable for any character the 8x8 face *does* cover, which is
    // the opposite of a fallback chain's purpose.
    #[cfg(feature = "fonts-cjk-bitmap")]
    static WITH_CJK: [&dyn GlyphSource; 2] =
        [&cjk::CjkBitmapSource::INSTANCE, &Font8x8Source::INSTANCE];

    #[cfg(feature = "fonts-cjk-bitmap")]
    let stack = FontStack::new(&WITH_CJK);
    #[cfg(not(feature = "fonts-cjk-bitmap"))]
    let stack = FontStack::new(&BASE);
    stack
}

/// Resolve `ch` through the active stack, falling back to tofu.
pub fn resolve(ch: char) -> (GlyphBitmap, Option<&'static str>) {
    active_stack().resolve_or_tofu(ch)
}

/// Which source answers `ch` on this build, or `None` for tofu.
///
/// Exposed so a test can assert the resolution *path* without inspecting pixels.
pub fn source_for(ch: char) -> Option<&'static str> {
    active_stack().resolve(ch).map(|(source, _)| source.name())
}

#[cfg(feature = "fonts-cjk-bitmap")]
pub mod cjk {
    //! The opt-in 16x16 CJK bitmap face.
    //!
    //! Glyph data is a generated subset of GNU Unifont — see the repository-root `NOTICE` and the
    //! generated file's own header. It is read **on demand** from the binary's read-only section:
    //! a binary search for the codepoint, then a 32-byte window, so the table is never resident.

    use super::{BitOrder, GlyphBitmap, GlyphSource};

    /// Bytes per glyph: 16 rows of 2 bytes.
    const GLYPH_BYTES: usize = 32;

    /// The 16x16 bitmap face over the generated Unifont subset.
    pub struct CjkBitmapSource;

    impl CjkBitmapSource {
        /// The one shared instance.
        pub const INSTANCE: Self = Self;
    }

    impl GlyphSource for CjkBitmapSource {
        fn glyph(&self, ch: char) -> Option<GlyphBitmap> {
            let codepoint = ch as u32;
            // Binary search, not a linear scan: the table covers thousands of ideographs and a
            // label is measured per glyph per frame.
            let index =
                crate::render::text::cjk_bitmap_data::CODEPOINTS.binary_search(&codepoint).ok()?;
            let start = index.checked_mul(GLYPH_BYTES)?;
            let end = start.checked_add(GLYPH_BYTES)?;
            // A sub-range of a `static` is itself `'static`, so this borrows the table's own
            // storage — 32 bytes of it — and the rest of the 75 KB stays untouched.
            let rows = crate::render::text::cjk_bitmap_data::ROWS.get(start..end)?;
            Some(GlyphBitmap::borrowed(16, 16, BitOrder::MsbFirst, rows))
        }

        fn name(&self) -> &'static str {
            "cjk-bitmap"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ascii_character_resolves_through_the_8x8_face() {
        let (glyph, source) = resolve('A');
        assert_eq!(source, Some("font8x8"), "Latin is the base face's job");
        assert_eq!((glyph.width, glyph.height), (8, 8));
        assert!(!glyph.bit(0, 0), "the 'A' cell's corner is empty");
    }

    #[test]
    fn whitespace_and_missing_characters_still_resolve_to_something_drawable() {
        // The resolver never returns "nothing": a caller always has a bitmap, and the *source*
        // is `None` exactly when tofu was used.
        let (glyph, source) = resolve('\u{10FFFF}');
        assert_eq!(source, None, "a noncharacter has no glyph in any face");
        assert_eq!((glyph.width, glyph.height), (8, 8), "the tofu cell is 8x8");
    }

    #[test]
    fn a_source_that_does_not_cover_a_character_returns_none() {
        // This is what makes a stack possible. If Font8x8 answered `Some` with a box for CJK, the
        // CJK face would never be reached.
        assert!(Font8x8Source::INSTANCE.glyph('中').is_none());
        assert!(!Font8x8Source::INSTANCE.covers('中'));
        assert!(Font8x8Source::INSTANCE.covers('A'));
    }

    #[test]
    fn the_stack_prefers_its_first_covering_source() {
        struct Only(char);
        impl GlyphSource for Only {
            fn glyph(&self, ch: char) -> Option<GlyphBitmap> {
                (ch == self.0).then(|| GlyphBitmap::inline8([0xFF; 8]))
            }
            fn name(&self) -> &'static str {
                "only"
            }
        }
        static FIRST: Only = Only('x');
        static SECOND: Only = Only('y');
        static SOURCES: [&dyn GlyphSource; 2] = [&FIRST, &SECOND];
        let stack = FontStack::new(&SOURCES);

        assert_eq!(stack.resolve('x').map(|(s, _)| s.name()), Some("only"));
        assert_eq!(stack.resolve('y').map(|(s, _)| s.name()), Some("only"));
        assert!(stack.resolve('z').is_none());
    }

    #[test]
    fn bit_order_reads_both_conventions_correctly() {
        // Leftmost pixel set. The two faces encode that differently, and both must read as the
        // same pixel — otherwise one table renders mirrored.
        let lsb = GlyphBitmap::inline8([0b0000_0001, 0, 0, 0, 0, 0, 0, 0]);
        let msb = GlyphBitmap::borrowed(8, 1, BitOrder::MsbFirst, &[0b1000_0000]);
        assert!(lsb.bit(0, 0) && !lsb.bit(7, 0), "LSB-first: bit 0 is leftmost");
        assert!(msb.bit(0, 0) && !msb.bit(7, 0), "MSB-first: the high bit is leftmost");
    }

    #[test]
    fn out_of_bounds_reads_are_blank_rather_than_a_panic() {
        let glyph = GlyphBitmap::inline8([0xFF; 8]);
        assert!(!glyph.bit(8, 0), "past the right edge");
        assert!(!glyph.bit(0, 8), "past the bottom edge");
        // A table shorter than its declared size must render blank, not panic.
        let short = GlyphBitmap::borrowed(16, 16, BitOrder::MsbFirst, &[0xFF, 0xFF]);
        assert!(short.bit(0, 0));
        assert!(!short.bit(0, 5), "beyond the supplied rows");
    }

    #[cfg(feature = "fonts-cjk-bitmap")]
    #[test]
    fn the_cjk_face_covers_an_ideograph_and_the_stack_finds_it() {
        assert_eq!(source_for('中'), Some("cjk-bitmap"), "the CJK face must answer for CJK");
        assert_eq!(source_for('A'), Some("font8x8"), "and must not shadow Latin");
        let (glyph, _) = resolve('中');
        assert_eq!((glyph.width, glyph.height), (16, 16), "the CJK cell is 16x16");

        // The pixels, not just the presence, are the point: this is the generated table's
        // end-to-end proof — correct binary-search index, correct 32-byte window, and
        // MSB-first row order. U+4E2D 中 is a vertical stroke through a boxed 口, so row 0
        // is one lit pixel at x=7 and row 4 is the box's top bar across x=2..=12. A wrong
        // byte order or a one-glyph index slip would light a different pattern.
        let ink: [u16; 16] = core::array::from_fn(|y| {
            (0..16u32)
                .fold(0u16, |acc, x| if glyph.bit(x, y as u32) { acc | (1 << x) } else { acc })
        });
        assert_eq!(ink[0], 1 << 7, "the vertical stroke's tip");
        assert_eq!(ink[4], 0x1FFC, "the top bar of the boxed radical");
        assert_eq!(ink[15], 1 << 7, "and the stroke continues to the last row");
    }
}
