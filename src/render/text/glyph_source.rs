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
//!
//! # Why a face *paints into a buffer* instead of returning pixels
//!
//! A 1-bit table's rows can be borrowed forever: they are in the binary. A vector face's pixels
//! cannot be — they are computed for one cell size, at one moment, and must not become resident
//! (the third constraint: a headless profile cannot hold thousands of rasterised glyphs). The
//! two are therefore expressed the same way, as *"write your coverage into this buffer"*, and
//! the caller owns the buffer.
//!
//! That single change is what makes the next two capabilities possible **without** touching this
//! module's shape again:
//!
//! * a vector face writes antialiased **coverage** (`0..=255`) into the same `cell.area()` bytes
//!   a 1-bit face writes `0` or `255` into — one buffer, one blend, no second code path;
//! * a colour face writes four bytes per pixel instead of one, which is why [`Painted`] reports
//!   what it produced rather than the caller guessing.
//!
//! [`GlyphBitmap`] survives as the *1-bit view* of a face, because two consumers genuinely need
//! the bits rather than the coverage: the resolution chain ([`FontStack::resolve`]) and a vector
//! backend that compresses by source pixel (a set source bit is one rectangle, which is smaller
//! than one rectangle per destination pixel).

/// A pixel box a glyph is painted into, in device pixels.
///
/// The cell is **not** the glyph's own size: it is the box the caller has decided to draw in
/// (for a label, the cluster's advance wide and the line box tall). A 1-bit face scales its
/// source cell into it; a vector face rasterises at it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cell {
    /// Width in device pixels.
    pub width: u32,
    /// Height in device pixels.
    pub height: u32,
}

impl Cell {
    /// A cell of `width x height` device pixels.
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    /// The number of pixels — and therefore the number of coverage bytes [`GlyphSource::paint`]
    /// writes.
    pub const fn area(self) -> usize {
        self.width as usize * self.height as usize
    }

    /// Whether the cell has no pixels, in which case painting is a no-op.
    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

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

/// What a face actually produced, so a caller can tell *which* face answered and how.
///
/// # Why the caller is told rather than left to guess
///
/// A renderer has to know **what shape of ink** arrived, because the same buffer can now hold
/// three different things and only one of them is the crate's historical 1-bit coverage:
///
/// * a 1-bit face writes `0` or `255` — a bitmap backend may emit one rectangle per source pixel;
/// * a vector face writes a coverage ramp `0..=255` — the rasteriser blends it directly and a
///   vector backend can no longer compress by source pixel, because there is no source pixel grid;
/// * a colour face writes **four** bytes per pixel — the rasteriser must stop treating the buffer
///   as coverage and start treating it as RGBA.
///
/// Reporting the shape is what makes all three reachable through one call site without a second
/// entry point, a downcast, or a guess. `Painted::is_coverage` and `Painted::is_color` are the two
/// questions a renderer actually asks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Painted {
    /// The name of the source that answered ([`GlyphSource::name`]).
    pub source: &'static str,
    /// The 1-bit source cell the ink was scaled from, when there was one.
    ///
    /// A vector backend uses this to emit **one rectangle per source pixel** rather than one per
    /// destination pixel: for an 8x8 glyph scaled into a 40x40 box the difference is 30 subpaths
    /// against 750. `None` means the ink is not 1-bit, which a bitmap-compressing backend must
    /// handle the other way.
    pub source_cell: Option<Cell>,
    /// What the bytes written into the caller's buffer *are*.
    pub ink: InkKind,
}

/// What shape of ink [`GlyphSource::paint`] produced.
///
/// The buffer is the caller's and its length is `cell.area()` pixels in every case; what differs is
/// how many bytes per pixel and what they mean. Named as an enum rather than a `bool` per shape
/// because the shapes are mutually exclusive — a face produces exactly one of them — and a pair of
/// flags would admit the impossible `color && coverage` state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InkKind {
    /// One byte per pixel, `0` or `255`: the crate's historical 1-bit ink.
    ///
    /// This is the only kind for which [`Painted::source_cell`] is meaningful, because it is the
    /// only kind that came from a source pixel grid.
    Bitmap,
    /// One byte per pixel, `0..=255`: antialiased coverage from a vector outline or a scaled face.
    ///
    /// A caller blends it as alpha exactly as it blends a unit-scale 1-bit ramp; the difference is
    /// that the ramp has interior values, which a bitmap backend cannot compress into rectangles.
    Coverage,
    /// **Four** bytes per pixel, premultiplied-ready straight RGBA: a colour glyph (a CBDT/sbix
    /// bitmap or a COLR layer stack).
    ///
    /// The buffer therefore has to be `cell.area() * 4` bytes for this kind, which is the one case
    /// where "the buffer is one byte per pixel" stops holding. A caller that only reserved
    /// `cell.area()` bytes is told `None` rather than having its buffer overrun.
    Color,
}

impl Painted {
    /// Bytes per pixel this ink occupies.
    pub const fn bytes_per_pixel(self) -> usize {
        match self.ink {
            InkKind::Bitmap | InkKind::Coverage => 1,
            InkKind::Color => 4,
        }
    }

    /// The buffer size [`GlyphSource::paint`] needs for this kind at `cell`.
    pub const fn buffer_len(self, cell: Cell) -> usize {
        cell.area() * self.bytes_per_pixel()
    }

    /// Whether the ink is a per-pixel alpha ramp (so the rasteriser can blend it as-is).
    pub const fn is_coverage(self) -> bool {
        matches!(self.ink, InkKind::Bitmap | InkKind::Coverage)
    }

    /// Whether the ink carries its own colour, so the caller's text colour must not be applied.
    pub const fn is_color(self) -> bool {
        matches!(self.ink, InkKind::Color)
    }

    /// A 1-bit report from a named source over a source cell, or the tofu fallback.
    const fn bitmap(source: &'static str, source_cell: Option<Cell>) -> Self {
        Self { source, source_cell, ink: InkKind::Bitmap }
    }
}

/// Scale a 1-bit glyph into `cell`, writing one coverage byte per cell pixel.
///
/// # The one placement rule for 1-bit faces
///
/// A source pixel `gx` covers the destination columns `gx * w / gw .. (gx + 1) * w / gw` —
/// nearest-neighbour, with integer arithmetic. That is not an approximation invented here: it is
/// the exact expression the two renderers used when each walked the bitmap itself, so a face that
/// paints through this function produces the same pixels the crate always drew, and the crate's
/// 376 committed snapshots do not move. Stated once, tested once, used by every 1-bit face.
///
/// Returns whether any ink was written. `out` must be at least `cell.area()` bytes; a shorter
/// buffer is refused rather than written past (a drawing routine is the wrong place to discover a
/// sizing bug).
pub fn paint_bitmap(glyph: &GlyphBitmap, cell: Cell, out: &mut [u8]) -> bool {
    if cell.is_empty() {
        return false;
    }
    let needed = cell.area();
    if out.len() < needed {
        return false;
    }
    // A caller reuses one buffer for every glyph, so the previous glyph's ink has to go.
    out[..needed].fill(0);

    let (width, height) = (cell.width as i32, cell.height as i32);
    let gw = glyph.width.max(1) as i32;
    let gh = glyph.height.max(1) as i32;
    let mut any = false;
    for gy in 0..gh {
        if !(0..gh).contains(&gy) {
            continue;
        }
        // The rows a source row covers, and the columns a source column covers. A zero-extent
        // range (`w < gw`) is widened to one pixel, exactly as the old rectangle walk did.
        let y0 = gy * height / gh;
        let y1 = ((gy + 1) * height / gh).max(y0 + 1).min(height);
        for gx in 0..gw {
            if !glyph.bit(gx as u32, gy as u32) {
                continue;
            }
            let x0 = gx * width / gw;
            let x1 = ((gx + 1) * width / gw).max(x0 + 1).min(width);
            for y in y0..y1 {
                for x in x0..x1 {
                    out[(y * width + x) as usize] = 255;
                    any = true;
                }
            }
        }
    }
    any
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

    /// Paint `ch`'s coverage into `out`, one byte per pixel of `cell`, row-major.
    ///
    /// `out` must be at least `cell.area()` bytes for a coverage or bitmap face, and
    /// `cell.area() * 4` for a colour face; a buffer too short for what this face produces is
    /// refused with `None` rather than written past. `None` also means this face does not cover
    /// `ch` (so a stack falls through) or could not paint; a face that covers the character but has
    /// no ink for it (a space) returns `Some` with the buffer left blank, which is the honest
    /// report — "this character is mine, and it is empty".
    ///
    /// The default implementation is the 1-bit path: take the glyph, scale it by
    /// [`paint_bitmap`]. A face whose ink is not 1-bit (a vector outline, a colour bitmap)
    /// overrides this and writes its own coverage, and reports which with [`Painted::ink`].
    fn paint(&self, ch: char, cell: Cell, out: &mut [u8]) -> Option<Painted> {
        let glyph = self.glyph(ch)?;
        let source_cell = Cell::new(glyph.width, glyph.height);
        paint_bitmap(&glyph, cell, out);
        Some(Painted::bitmap(self.name(), Some(source_cell)))
    }
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

/// The opt-in colour emoji face: `CBDT`/`CBLC` bitmap glyphs, each a PNG (G-6).
///
/// # Why this overrides `paint` and returns `None` from `glyph`
///
/// Every other source in this module answers both questions — "what bits are you?" ([`Self::glyph`])
/// and "what does it look like in this cell?" ([`Self::paint`]). A colour face can only answer the
/// second: its ink is colour RGBA at whatever cell the caller asked for, so there is no 1-bit view to
/// return, and the value is computed per call rather than borrowed from the binary.
///
/// That is not a gap. [`Self::glyph`] is documented as the **1-bit view**, and a resolution chain
/// that wants a bitmap will simply not use this face; [`Self::paint`] is the path a renderer takes,
/// and a colour face reports [`InkKind::Color`] so the renderer knows the bytes are RGBA.
#[cfg(feature = "fonts-emoji-color")]
pub struct ColorBitmapSource;

#[cfg(feature = "fonts-emoji-color")]
impl ColorBitmapSource {
    /// The one shared instance.
    pub const INSTANCE: Self = Self;

    /// The colour face that covers `ch`, if this build enabled one.
    fn face_for(&self, ch: char) -> Option<crate::render::text::font_assets::ColorFaceBytes> {
        let codepoint = ch as u32;
        crate::render::text::font_assets::active_color_faces()
            .iter()
            .copied()
            .find(|face| has_glyph(face.bytes, codepoint))
    }
}

/// Whether `face` has a glyph for `codepoint`.
///
/// Parsed per query rather than cached: a `CBDT` glyph is looked up once per cluster per frame, the
/// face directory is 13 entries, and caching would need either a global or a lifetime threaded
/// through every call — for a table this size neither is worth it.
#[cfg(feature = "fonts-emoji-color")]
fn has_glyph(face: &[u8], codepoint: u32) -> bool {
    ttf_parser::Face::parse(face, 0)
        .ok()
        .and_then(|parsed| parsed.glyph_index(char::from_u32(codepoint)?))
        .is_some()
}

#[cfg(feature = "fonts-emoji-color")]
impl GlyphSource for ColorBitmapSource {
    fn glyph(&self, ch: char) -> Option<GlyphBitmap> {
        // A colour glyph has no 1-bit view. Answering `None` is the honest report: the ink is RGBA
        // and it depends on the cell. See this type's own docs.
        let _ = self.face_for(ch);
        None
    }

    fn name(&self) -> &'static str {
        "color-emoji"
    }

    fn covers(&self, ch: char) -> bool {
        self.face_for(ch).is_some()
    }

    fn paint(&self, ch: char, cell: Cell, out: &mut [u8]) -> Option<Painted> {
        let face_bytes = self.face_for(ch)?;
        let parsed = ttf_parser::Face::parse(face_bytes.bytes, 0).ok()?;
        let glyph_id = parsed.glyph_index(ch)?;
        let face = crate::render::text::ColorBitmapFace::parse(face_bytes.bytes)?;
        // The buffer check and the buffer *use* are deliberately adjacent: `face.paint` also checks,
        // but checking here means a caller that handed a 1-bit-sized buffer gets the honest `None`
        // (and therefore the fall-through to the 1-bit face behind this one) from the same place
        // that located the glyph, rather than after a failed lookup.
        //
        // A caller that *can* take colour ink sizes its buffer with `cell.area() * 4`; the same
        // contract reaches the host through `render::text::InkKind::Color`.
        if cell.is_empty() || out.len() < cell.area() * 4 {
            return None;
        }
        face.paint(glyph_id.0, cell, out)?;
        Some(Painted {
            source: face_bytes.name,
            // There is no source pixel grid to compress by: this ink is colour, not a bitmap ramp.
            source_cell: None,
            ink: InkKind::Color,
        })
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
    #[cfg(not(any(
        feature = "fonts-cjk-bitmap",
        feature = "fonts-vector-latin",
        feature = "fonts-complex",
        feature = "fonts-emoji-color"
    )))]
    static BASE: [&dyn GlyphSource; 1] = [&Font8x8Source::INSTANCE];

    // The stack with the CJK bitmap face added.
    //
    // The CJK face comes **first** for the characters it covers, because it is the only face
    // that has them; the 8x8 face stays last so Latin resolution is untouched. Ordering it the
    // other way (8x8 first) would be harmless for Latin — the 8x8 face does not cover CJK — but
    // would leave the CJK face unreachable for any character the 8x8 face *does* cover, which is
    // the opposite of a fallback chain's purpose.
    #[cfg(any(feature = "fonts-vector-latin", feature = "fonts-complex"))]
    static WITH_VECTOR: [&dyn GlyphSource; 2] =
        [&super::raster::VectorSource::INSTANCE, &Font8x8Source::INSTANCE];

    #[cfg(feature = "fonts-cjk-bitmap")]
    static WITH_CJK: [&dyn GlyphSource; 2] =
        [&cjk::CjkBitmapSource::INSTANCE, &Font8x8Source::INSTANCE];

    #[cfg(feature = "fonts-emoji-color")]
    static WITH_EMOJI: [&dyn GlyphSource; 2] =
        [&ColorBitmapSource::INSTANCE, &Font8x8Source::INSTANCE];

    // The three-way stacks. A colour emoji face goes **first**: it is the only source for a
    // character outside the text faces, and the text faces answer tofu for those — so putting it
    // later would make it unreachable for exactly the characters it exists for.
    #[cfg(all(feature = "fonts-emoji-color", not(feature = "fonts-cjk-bitmap")))]
    let stack = FontStack::new(&WITH_EMOJI);

    #[cfg(all(feature = "fonts-emoji-color", feature = "fonts-cjk-bitmap"))]
    static WITH_EMOJI_AND_CJK: [&dyn GlyphSource; 3] =
        [&ColorBitmapSource::INSTANCE, &cjk::CjkBitmapSource::INSTANCE, &Font8x8Source::INSTANCE];
    #[cfg(all(feature = "fonts-emoji-color", feature = "fonts-cjk-bitmap"))]
    let stack = FontStack::new(&WITH_EMOJI_AND_CJK);

    #[cfg(all(
        feature = "fonts-cjk-bitmap",
        any(feature = "fonts-vector-latin", feature = "fonts-complex")
    ))]
    static WITH_CJK_AND_VECTOR: [&dyn GlyphSource; 3] = [
        &cjk::CjkBitmapSource::INSTANCE,
        &super::raster::VectorSource::INSTANCE,
        &Font8x8Source::INSTANCE,
    ];
    #[cfg(all(
        feature = "fonts-cjk-bitmap",
        any(feature = "fonts-vector-latin", feature = "fonts-complex"),
        not(feature = "fonts-emoji-color")
    ))]
    let stack = FontStack::new(&WITH_CJK_AND_VECTOR);

    #[cfg(all(
        feature = "fonts-cjk-bitmap",
        any(feature = "fonts-vector-latin", feature = "fonts-complex"),
        feature = "fonts-emoji-color"
    ))]
    static ALL_FACES: [&dyn GlyphSource; 4] = [
        &ColorBitmapSource::INSTANCE,
        &cjk::CjkBitmapSource::INSTANCE,
        &super::raster::VectorSource::INSTANCE,
        &Font8x8Source::INSTANCE,
    ];
    #[cfg(all(
        feature = "fonts-cjk-bitmap",
        any(feature = "fonts-vector-latin", feature = "fonts-complex"),
        feature = "fonts-emoji-color"
    ))]
    let stack = FontStack::new(&ALL_FACES);

    #[cfg(all(
        feature = "fonts-cjk-bitmap",
        not(any(feature = "fonts-vector-latin", feature = "fonts-complex")),
        not(feature = "fonts-emoji-color")
    ))]
    let stack = FontStack::new(&WITH_CJK);

    #[cfg(all(
        not(feature = "fonts-cjk-bitmap"),
        any(feature = "fonts-vector-latin", feature = "fonts-complex"),
        not(feature = "fonts-emoji-color")
    ))]
    let stack = FontStack::new(&WITH_VECTOR);

    #[cfg(all(
        not(feature = "fonts-cjk-bitmap"),
        feature = "fonts-emoji-color",
        any(feature = "fonts-vector-latin", feature = "fonts-complex")
    ))]
    static EMOJI_AND_VECTOR: [&dyn GlyphSource; 3] = [
        &ColorBitmapSource::INSTANCE,
        &super::raster::VectorSource::INSTANCE,
        &Font8x8Source::INSTANCE,
    ];
    #[cfg(all(
        not(feature = "fonts-cjk-bitmap"),
        feature = "fonts-emoji-color",
        any(feature = "fonts-vector-latin", feature = "fonts-complex")
    ))]
    let stack = FontStack::new(&EMOJI_AND_VECTOR);

    #[cfg(not(any(
        feature = "fonts-cjk-bitmap",
        feature = "fonts-vector-latin",
        feature = "fonts-complex",
        feature = "fonts-emoji-color"
    )))]
    let stack = FontStack::new(&BASE);

    stack
}

/// Resolve `ch` through the active stack, falling back to tofu.
pub fn resolve(ch: char) -> (GlyphBitmap, Option<&'static str>) {
    active_stack().resolve_or_tofu(ch)
}

/// Paint `ch`'s coverage through the active stack, falling back to tofu.
///
/// This is [`resolve`]'s sibling for a renderer that draws *pixels* rather than rectangles, and
/// the two agree by construction: the tofu fallback paints the same 8x8 block `resolve` returns,
/// and a covering face paints its own ink either way. `None` means the buffer was too small.
///
/// # Buffer sizing is the caller's contract, and a colour face needs four bytes per pixel
///
/// Most faces here are 1-bit coverage: one byte per pixel, so `cell.area()` is the right size. A
/// colour face ([`InkKind::Color`]) writes RGBA — four bytes per pixel — so the same cell needs
/// `cell.area() * 4`. A caller that reserved only `cell.area()` still gets its pixels, just not
/// from the colour face: the source refuses the short buffer and the stack falls through to the
/// 1-bit face behind it rather than a slice of the caller's buffer being overwritten past its end.
/// Use [`Painted::buffer_len`] to size a buffer when colour ink is possible.
pub fn paint_active(ch: char, cell: Cell, out: &mut [u8]) -> Option<Painted> {
    let stack = active_stack();
    for source in stack.sources() {
        if let Some(painted) = source.paint(ch, cell, out) {
            return Some(painted);
        }
    }
    // No face covers the character: tofu, exactly as `resolve` would return it. Tofu is 1-bit, so a
    // buffer sized for colour is truncated to its first `cell.area()` bytes by `paint_bitmap`.
    let tofu = GlyphBitmap::inline8(TOFU);
    paint_bitmap(&tofu, cell, out);
    Some(Painted::bitmap("tofu", Some(Cell::new(8, 8))))
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

    /// The 1-bit view and the painted coverage are the **same picture**, pixel for pixel.
    ///
    /// This is the assertion that makes it safe for the rasteriser to ask faces to *paint* while
    /// the SVG backend keeps asking them for **rectangles**: if the two derivations ever disagreed,
    /// a control's raster and its snapshot would be two different drawings, and neither the snapshot
    /// gate nor any test could say which was right. The reverse holds too — the rasteriser now
    /// blends real coverage, so anything other than `0`/`255` here would mean antialiasing had
    /// arrived on a face that has none.
    #[test]
    fn the_bitmap_view_and_the_painted_coverage_agree() {
        // A few cells per glyph: smaller than the 8x8 source, exactly it, and larger on both axes.
        for ch in ['A', 'g', '5', '\u{4e2d}', '\u{10ffff}'] {
            for cell in [Cell::new(8, 8), Cell::new(5, 13), Cell::new(19, 23), Cell::new(64, 64)] {
                let mut coverage = vec![0u8; cell.area()];
                paint_active(ch, cell, &mut coverage);

                // Every pixel the rectangle walk fills must be fully covered, and every pixel it
                // leaves out must be empty. `ch.is_whitespace()` short-circuits both paths.
                let mut expected = vec![0u8; cell.area()];
                if !ch.is_whitespace() {
                    for (x0, y0, x1, y1) in
                        crate::render::glyph_rects(ch, 0, 0, cell.width, cell.height)
                    {
                        for y in y0..y1 {
                            for x in x0..x1 {
                                expected[(y * cell.width as i32 + x) as usize] = 255;
                            }
                        }
                    }
                }
                assert_eq!(
                    coverage, expected,
                    "U+{:04X} in {}x{}: painted coverage and the bitmap view must be one picture",
                    ch as u32, cell.width, cell.height
                );
            }
        }
    }

    /// A buffer shorter than the cell is refused rather than written past.
    ///
    /// A paint routine is the wrong place to discover a caller's sizing bug: writing past the end
    /// of a slice is undefined, and a drawing call has no way to report it.
    #[test]
    fn a_short_buffer_is_refused_rather_than_written_past() {
        let cell = Cell::new(8, 8);
        let mut short = [0u8; 63];
        assert!(!paint_bitmap(&GlyphBitmap::inline8([0xFF; 8]), cell, &mut short));
        assert!(short.iter().all(|byte| *byte == 0), "nothing may be written");
    }
}
