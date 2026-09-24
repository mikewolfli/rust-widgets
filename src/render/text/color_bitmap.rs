// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Reading a colour bitmap glyph out of a `CBDT`/`CBLC` face — G-6.
//!
//! # Why this is a different kind of face from every other one here
//!
//! G-1's shaper and G-5's rasteriser both read *outlines*: curves in design units, scaled to
//! whatever cell the caller wants. A colour emoji face has no outlines at all. Its glyphs are
//! **PNGs**, one per glyph, rendered by the designer at exactly one size, indexed by two tables:
//!
//! * `CBLC` — where each glyph's image is (an index of `(glyph, offset, length)`);
//! * `CBDT` — the images themselves, each a PNG blob.
//!
//! That is why the third [`crate::render::text::InkKind`] exists and why this module is separate:
//! there is no scale to apply, no coverage to derive, and the bytes are colour, not alpha.
//!
//! # The three things that make this unlike parsing an outline
//!
//! 1. **The image arrives at one size.** `CBDT` declares its strike's `ppem`, and every bitmap in it
//!    was rendered at that size. A caller that asked for a different cell gets the bitmap *scaled* —
//!    nearest-neighbour, because a bitmap has no outline to re-rasterise and bilinear filtering on a
//!    109 px emoji is what makes it look soft.
//! 2. **The pixels are compressed.** Each image is a PNG, so this module needs a decoder. The crate
//!    already depends on `miniz_oxide` for other image work, but a PNG needs more than inflate:
//!    filters, bit depths, palette and alpha. That decoder lives in [`super::png`] and is written
//!    against the PNG spec rather than pulled in, so the text layer keeps its dependencies.
//! 3. **A glyph may legitimately have no bitmap.** Emoji are often composed: `👨‍👩‍👧` is one
//!    cluster, but a face may carry the individuals and no combined image. `None` from
//!    [`ColorBitmap::image`] means "no image for this glyph", which is the honest answer that lets a
//!    caller fall through.
//!
//! # Why there is no allocation for a miss
//!
//! A lookup returns a borrowed slice of the face's own bytes. Only a *hit* that has to be decoded and
//! scaled touches a buffer, and that buffer is the caller's.

use super::png::decode_png;

/// The eight bytes every PNG starts with.
const PNG_SIGNATURE: &[u8] = &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// `BigGlyphMetrics`' length, which format 2's stride includes.
const BIG_GLYPH_METRICS_LEN: usize = 8;

/// `SmallGlyphMetrics`' length, which formats 17's and 2's strides include.
const SMALL_GLYPH_METRICS_LEN: usize = 5;

/// Header bytes ahead of the PNG in each `CBDT` image format.
///
/// | format | header before the PNG |
/// |---|---|
/// | 17 | `SmallGlyphMetrics` (5) + `dataLen` u32 (4) = 9 |
/// | 18 | `BigGlyphMetrics` (8) + `dataLen` u32 (4) = 12 |
/// | 19 | `dataLen` u32 (4) = 4 |
///
/// The `dataLen` is *not* inferred from the index subtable's offset pair. It is read from the image
/// itself, because the two are only equal when the subtable is exact: format 2 has a constant stride
/// and pads nothing, and format 1's offsets may include padding that `dataLen` excludes. Trusting the
/// offset pair instead is what made the first attempt at this file slice a PNG blob four bytes early
/// — the four bytes `00 00 02 f2` in front of every image are exactly this field.
const _: () = ();

/// `CBDT`/`CBLC` table versions this supports.
///
/// v2 and v3 differ only in `CBLC`'s `SbitLineMetrics` layout (v3 added the vertical pair), which is
/// read past rather than parsed here. Refusing an unknown version is what keeps this from silently
/// mis-reading a future table layout — the failure mode a bitmap parser is worst at, because the
/// bytes still look like numbers.
const SUPPORTED_MAJOR: u16 = 3;

/// A colour bitmap face over borrowed bytes.
///
/// Holds the whole face and locates tables on each query. Parsing the directory is a handful of
/// bounds-checked reads, so caching it would mean either a global or a lifetime threaded through
/// every call — for a table of 13 entries, neither is worth it.
#[derive(Clone, Copy)]
pub struct ColorBitmapFace<'a> {
    data: &'a [u8],
}

/// Where one glyph's PNG lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageRef {
    /// Byte offset of the PNG **within `CBDT`**.
    pub offset: usize,
    /// Byte length of the PNG.
    pub length: usize,
}

impl<'a> ColorBitmapFace<'a> {
    /// Parses `data` as a face, returning `None` when it carries no usable colour bitmap tables.
    ///
    /// Returning `None` rather than an error type is deliberate: every caller of this either has a
    /// fallback face or has nothing to draw, and a colour face that cannot be read is
    /// indistinguishable from one that is absent at the point of use.
    pub fn parse(data: &'a [u8]) -> Option<Self> {
        let face = Self { data };
        // Both tables are required: `CBLC` without `CBDT` has indices into nothing.
        face.table(b"CBLC")?;
        face.table(b"CBDT")?;
        Some(face)
    }

    /// The `(offset, length)` of a table by tag, or `None`.
    fn table(&self, tag: &[u8; 4]) -> Option<(usize, usize)> {
        let count = read_u16(self.data, 4)? as usize;
        for index in 0..count {
            let entry = 12 + index * 16;
            let entry_tag = self.data.get(entry..entry + 4)?;
            if entry_tag != tag {
                continue;
            }
            let offset = read_u32(self.data, entry + 8)? as usize;
            let length = read_u32(self.data, entry + 12)? as usize;
            // Bounds-check once here, so every later read of this table can be relative.
            self.data.get(offset..offset.checked_add(length)?)?;
            return Some((offset, length));
        }
        None
    }

    /// The strike's `(ppem_x, ppem_y, start_glyph, end_glyph)`, or `None`.
    ///
    /// # The layout, and the offset bases that had to be verified
    ///
    /// `CBLC` is an 8-byte header (`major`, `minor`, `numSizes`) followed by 48-byte
    /// `BitmapSizeTable`s:
    ///
    /// | offset | field |
    /// |---|---|
    /// | +0 | `indexSubTableArrayOffset` u32 |
    /// | +4 | `indexTablesSize` u32 |
    /// | +8 | `numberOfIndexSubTables` u32 |
    /// | +12 | `colorRef` u32 |
    /// | +16..+28 | `hori` `SbitLineMetrics` |
    /// | +28..+40 | `vert` `SbitLineMetrics` |
    /// | +40 | `startGlyphIndex` u16 |
    /// | +42 | `endGlyphIndex` u16 |
    /// | +44 | `ppemX` u8, `ppemY` u8, `bitDepth` u8, `flags` i8 |
    ///
    /// Reading the size table as a flat `IIIIBBBB` from `CBLC+8` yields `glyphs 2..0 ppem=101x229
    /// depth=136` on this face: numbers that pass arithmetic and are all wrong. That is why the
    /// strike header is read field by field and why `tools/emoji_font_probe.py` prints it for
    /// inspection.
    ///
    /// # The two `additionalOffset` bases, which are the trap
    ///
    /// `BitmapSizeTable.indexSubTableArrayOffset` **and** `IndexSubTableArray.additionalOffsetTo-
    /// IndexSubtable` are both measured from the start of **`CBLC`**, not from `CBLC+8` and not from
    /// the array. The spec describes the first as "from the beginning of the bitmapSizeTables",
    /// which reads as table-relative, and reading it that way produces an array at `CBLC+8+56` whose
    /// records are `(1769750, 132, 65553)` — numbers that parse and mean nothing. The check that
    /// catches it is structural: the array at `CBLC+56` starts with a record whose `firstGlyphIndex`
    /// **equals the strike's own `startGlyphIndex`** (both 1 here), which no wrong base reproduces.
    fn strike(&self) -> Option<Strike> {
        let (cblc, _) = self.table(b"CBLC")?;
        let major = read_u16(self.data, cblc)?;
        if major != SUPPORTED_MAJOR {
            return None;
        }
        let num_sizes = read_u32(self.data, cblc + 4)?;
        if num_sizes == 0 {
            return None;
        }
        let base = cblc + 8;
        let index_array_offset = read_u32(self.data, base)? as usize;
        let index_count = read_u32(self.data, base + 8)? as usize;
        let start_glyph = read_u16(self.data, base + 40)?;
        let end_glyph = read_u16(self.data, base + 42)?;
        // `+44..+48` (ppemX, ppemY, bitDepth, flags) is read past deliberately: a colour bitmap is
        // scaled to the caller's `cell` rather than rendered at the strike's size, so the strike's
        // pixel size has no consumer. The offsets are named here so these docs and this function
        // cannot drift apart.
        let list = cblc.checked_add(index_array_offset)?;
        // The structural check: the first record must agree with the strike about its glyph range.
        // Without it a wrong base is indistinguishable from a face with an unusual layout, and the
        // failure is a silent "no image" for every glyph — which is exactly how the two candidate
        // bases above were told apart during development.
        let first_record_glyph = read_u16(self.data, list)?;
        if first_record_glyph != start_glyph {
            return None;
        }
        Some(Strike { list, index_count, start_glyph, end_glyph })
    }

    /// The PNG for `glyph_id`, or `None` when this face has no image for it.
    pub fn image(&self, glyph_id: u16) -> Option<ImageRef> {
        let strike = self.strike()?;
        if glyph_id < strike.start_glyph || glyph_id > strike.end_glyph {
            return None;
        }
        let (cbdt, cbdt_len) = self.table(b"CBDT")?;
        // Each record is `(firstGlyphIndex u16, lastGlyphIndex u16, indexSubtableOffset u32)`, and
        // `firstGlyphIndex` is a **glyph index**, so the record is found by ranging over them rather
        // than by indexing with the glyph id.
        for record in 0..strike.index_count {
            let entry = strike.list + record * 8;
            let first = read_u16(self.data, entry)?;
            let last = read_u16(self.data, entry + 2)?;
            if glyph_id < first || glyph_id > last {
                continue;
            }
            let subtable_offset = read_u32(self.data, entry + 4)? as usize;
            // The spec: "the indexSubtableOffset provides the location of the IndexSubtable. This can
            // be added to the indexSubtableListOffset ... to obtain the offset of the IndexSubtable
            // within the EBLC table." So it is measured from the **list**, not from `CBLC` — the
            // second of the two bases this table uses, and the one that is easiest to get wrong
            // because both are described as "offsets from the beginning of" a table.
            let subtable = strike.list.checked_add(subtable_offset)?;
            let index_format = read_u16(self.data, subtable)?;
            let image_format = read_u16(self.data, subtable + 2)?;
            let image_data_offset = read_u32(self.data, subtable + 4)? as usize;
            return self.locate_image(
                index_format,
                image_format,
                image_data_offset,
                subtable,
                first,
                last,
                glyph_id,
                cbdt,
                cbdt_len,
            );
        }
        None
    }

    /// Reads one glyph's `(offset, length)` out of an index subtable.
    ///
    /// # Why the image formats are checked, not assumed
    ///
    /// `imageFormat` says what the bytes *are*: 17/18 are PNG with metrics, 19 is a PNG with no
    /// metrics, and 1–9 are the legacy raw-bitmap formats the format was designed around. This crate
    /// only decodes PNG, so anything else is refused rather than fed to the PNG decoder — a legacy
    /// `imageFormat=1` blob would be misread as a PNG header and produce noise.
    ///
    /// # Why only formats 1 and 2 are implemented
    ///
    /// Format 2 is constant-metrics (an `imageSize` and one set of metrics, no offset array) and
    /// format 1 is variable-metrics with 4-byte offsets and **one extra entry** giving the last
    /// glyph's size — the two forms the shipped faces use. Formats 3/4/5 are the same idea with
    /// 2-byte offsets or sparse glyph lists; a glyph in one of them is reported as "no image", which
    /// is visible (a caller falls through) rather than wrong, and adding one is a self-contained
    /// change here.
    #[allow(clippy::too_many_arguments)]
    fn locate_image(
        &self,
        index_format: u16,
        image_format: u16,
        image_data_offset: usize,
        subtable: usize,
        first_glyph: u16,
        last_glyph: u16,
        glyph_id: u16,
        cbdt: usize,
        cbdt_len: usize,
    ) -> Option<ImageRef> {
        // 17 (small metrics + PNG), 18 (big metrics + PNG) and 19 (no metrics, PNG) are the only
        // colour formats that carry a PNG. The legacy 1–9 raw-bitmap formats are refused rather than
        // fed to the PNG decoder, which would read the first bytes of a bitmap as a signature.
        if image_format != 17 && image_format != 18 && image_format != 19 {
            return None;
        }
        let (image_start, header_len) = match index_format {
            // Format 1: `sbitOffsets[numOffsets]`, and `numOffsets = last - first + 2` — the extra
            // entry is what gives the final glyph its size, which is why it is read rather than
            // defaulted to the end of the table.
            1 => {
                let count = (last_glyph.checked_sub(first_glyph)? as usize) + 2;
                let index = glyph_id.checked_sub(first_glyph)? as usize;
                if index + 1 >= count {
                    return None;
                }
                let start = read_u32(self.data, subtable + 8 + index * 4)? as usize;
                let end = read_u32(self.data, subtable + 8 + (index + 1) * 4)? as usize;
                // The format spells "this glyph has no image" as a zero-length range. That is a
                // legitimate encodable state, so it is a miss rather than a malformed table.
                if end < start {
                    return None;
                }
                (start, image_format_header_len(image_format)?)
            }
            // Format 2: one `imageSize` for the whole range and one `BigGlyphMetrics` (8 bytes), so
            // the glyph's image is at a fixed stride and no offset array exists.
            2 => {
                let image_size = read_u32(self.data, subtable + 8)? as usize;
                let header = image_format_header_len(image_format)?;
                let stride = image_size.checked_add(BIG_GLYPH_METRICS_LEN)?;
                let index = glyph_id.checked_sub(first_glyph)? as usize;
                (index.checked_mul(stride)?, header)
            }
            _ => return None,
        };
        let image = cbdt.checked_add(image_data_offset)?.checked_add(image_start)?;

        // The three `CBDT` image formats 17/18/19 all put a `uint32 dataLen` immediately before the
        // PNG bytes, after the metrics. Reading it and then checking the signature at
        // `image + header_len` is what pins the offset down: a wrong `image_data_offset` base lands
        // somewhere else entirely and fails the signature rather than decoding to noise.
        let data_len =
            read_u32(self.data, image.checked_add(header_len)?.checked_sub(4)?)? as usize;
        if data_len == 0 {
            return None;
        }
        let png_offset = image.checked_add(header_len)?;
        let slice = self.data.get(png_offset..png_offset.checked_add(data_len)?)?;
        let _ = cbdt_len;
        // A PNG's eight-byte signature, checked here so a wrong offset is a miss rather than a
        // decode failure deep inside the inflate step.
        if !slice.starts_with(PNG_SIGNATURE) {
            return None;
        }
        Some(ImageRef { offset: png_offset, length: data_len })
    }

    /// Decodes the PNG for `glyph_id` and writes it into `out` as straight RGBA, scaled to `cell`.
    ///
    /// `out` must be at least `cell.area() * 4` bytes. The bitmap is scaled to `cell` by
    /// nearest-neighbour, because a colour bitmap has no outline to re-rasterise and filtering a
    /// 109 px emoji is what makes it look soft.
    ///
    /// # Why the decode goes through a scratch buffer
    ///
    /// [`decode_png`] writes RGBA into the buffer it is given and hands back a *view of it*, and the
    /// view's size is the PNG's own — the strike's size (109 px), not the cell's. The scale step
    /// reads one buffer and writes another, so they cannot be the same buffer: aliasing them would
    /// be a borrow error, and if it compiled it would have the scaler read pixels it had already
    /// overwritten.
    ///
    /// So the decoded image gets its own allocation and `out` receives only the scaled result. It is
    /// dropped at the end of the call, which is what keeps the third constraint: no colour glyph is
    /// ever resident.
    ///
    /// Returns `None` when the face has no image for the glyph, the PNG cannot be decoded, or the
    /// buffer is too small for the cell.
    pub fn paint(&self, glyph_id: u16, cell: super::Cell, out: &mut [u8]) -> Option<()> {
        let needed = cell.area() * 4;
        if cell.is_empty() || out.len() < needed {
            return None;
        }
        let image = self.image(glyph_id)?;
        let bytes = self.data.get(image.offset..image.offset.checked_add(image.length)?)?;

        // The decoded size is known from the PNG's own `IHDR` before decoding, so the scratch is
        // sized once rather than grown.
        let (width, height) = png_dimensions(bytes)?;
        let decoded_len = (width as usize).checked_mul(height as usize)?.checked_mul(4)?;
        let mut decoded = crate::compat::vec![0u8; decoded_len];
        let view = decode_png(bytes, &mut decoded)?;
        scale_rgba(view.pixels, view.width, view.height, cell, out);
        Some(())
    }
}

/// How many bytes sit between the start of a glyph's image in `CBDT` and its PNG bytes.
///
/// Format 17 carries `SmallGlyphMetrics` (5), format 18 carries `BigGlyphMetrics` (8), and format 19
/// — where the metrics live in `CBLC` instead — carries none. Each is followed by the `dataLen`
/// u32 that [`ColorBitmapFace::locate_image`] reads at `offset + header_len - 4`.
fn image_format_header_len(image_format: u16) -> Option<usize> {
    match image_format {
        17 => Some(SMALL_GLYPH_METRICS_LEN + 4),
        18 => Some(BIG_GLYPH_METRICS_LEN + 4),
        19 => Some(4),
        _ => None,
    }
}

/// The pixel size a PNG's `IHDR` declares, without decoding it.
///
/// Split out from [`decode_png`] because the caller has to size a scratch buffer first, and the size
/// is in the third and fourth `u32` of the file after the signature and the `IHDR` header.
fn png_dimensions(data: &[u8]) -> Option<(u32, u32)> {
    // signature (8) + length (4) + "IHDR" (4) + width (4) + height (4)
    let bytes = data.get(16..24)?;
    let width = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    let height = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if width == 0 || height == 0 {
        return None;
    }
    Some((width, height))
}

/// One `BitmapSizeTable`, reduced to what a lookup needs.
#[derive(Debug, Clone, Copy)]
struct Strike {
    /// The `IndexSubtableList`'s absolute offset: `CBLC + indexSubtableListOffset`.
    ///
    /// Both other offsets in this table are measured from **this**, not from `CBLC` — see
    /// [`ColorBitmapFace::image`].
    list: usize,
    /// `numberOfIndexSubtables`.
    index_count: usize,
    /// The strike's first glyph id.
    start_glyph: u16,
    /// The strike's last glyph id, inclusive.
    end_glyph: u16,
}

/// Copies `src` (RGBA) into `out` (RGBA) at `cell`, nearest-neighbour.
///
/// # Why nearest-neighbour and not a filter
///
/// A `CBDT` glyph is a designer's rendering, made at one size. Scaling it up with bilinear filtering
/// produces exactly the soft, over-smoothed look that makes a bitmap emoji read as "wrong" next to
/// text rendered from outlines. Nearest-neighbour keeps the original pixels' edges — and, more
/// importantly, is *predictable*: the same cell always yields the same bytes, which is what lets a
/// snapshot be a regression test.
fn scale_rgba(src: &[u8], src_w: u32, src_h: u32, cell: super::Cell, out: &mut [u8]) {
    let needed = cell.area() * 4;
    if out.len() < needed {
        return;
    }
    if src_w == 0 || src_h == 0 {
        out[..needed].fill(0);
        return;
    }
    for py in 0..cell.height {
        let sy = (py * src_h / cell.height).min(src_h - 1);
        for px in 0..cell.width {
            let sx = (px * src_w / cell.width).min(src_w - 1);
            let si = ((sy * src_w + sx) * 4) as usize;
            let di = ((py * cell.width + px) * 4) as usize;
            if let (Some(from), Some(to)) = (src.get(si..si + 4), out.get_mut(di..di + 4)) {
                to.copy_from_slice(from);
            }
        }
    }
}

/// Big-endian `u16` read, `None` past the end.
fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

/// Big-endian `u32` read, `None` past the end.
fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(all(test, feature = "fonts-emoji-color"))]
mod tests {
    use super::*;
    use crate::render::text::font_assets::EMOJI;

    /// The face parses, which means both `CBLC` and `CBDT` are present and readable.
    #[test]
    fn the_shipped_face_parses() {
        assert!(ColorBitmapFace::parse(EMOJI).is_some(), "the shipped emoji face must parse");
    }

    /// A colour face with no `CBLC` has indices into nothing and is refused.
    #[test]
    fn a_face_without_the_locator_table_is_refused() {
        assert!(ColorBitmapFace::parse(b"").is_none());
        assert!(ColorBitmapFace::parse(b"not a font").is_none());
    }

    /// A covered glyph resolves to a PNG, and the PNG really is one.
    ///
    /// This is the assertion that would have caught the original offset bug: a wrong
    /// `imageDataOffset` base returns `Some` with a plausible length and bytes that are not a PNG.
    ///
    /// The glyph id is looked up through the cmap rather than hard-coded, because the shipped subset
    /// is regenerated whenever the codepoint list changes and glyph ids move with it — the strike
    /// starts at 16 in the current subset, and pinning `1` here made this test fail on a subset that
    /// was in fact correct.
    #[test]
    fn a_covered_glyph_resolves_to_a_real_png() {
        let face = ColorBitmapFace::parse(EMOJI).expect("parse");
        let parsed = ttf_parser::Face::parse(EMOJI, 0).expect("ttf-parser face");
        let glyph_id = parsed.glyph_index('\u{1F600}').expect("the grinning face is in the subset");
        let image = face.image(glyph_id.0).expect("the grinning face has an image");
        let bytes = EMOJI
            .get(image.offset..image.offset + image.length)
            .expect("the image is inside the face");
        assert_eq!(
            &bytes[..8],
            PNG_SIGNATURE,
            "the located bytes must start with the PNG signature"
        );
        // `IHDR` must agree with `SmallGlyphMetrics`, which the index located: the strike is
        // 109 ppem and its glyphs are rendered at the exact pixel size they will be drawn at.
        let (width, height) = png_dimensions(bytes).expect("IHDR is readable");
        assert!(width > 0 && height > 0);
        assert!(width <= 200 && height <= 200, "sane strike size, got {width}x{height}");
    }

    /// Every glyph the cmap names must resolve to an image, or the subset is inconsistent.
    ///
    /// This is the iceberg check for the offset bug: a single wrong base makes *one* glyph look
    /// plausible and this count collapse, so ranging over the whole cmap is what makes the failure
    /// loud instead of a silent "some emoji do not draw".
    #[test]
    fn every_cmap_glyph_has_an_image() {
        let face = ColorBitmapFace::parse(EMOJI).expect("parse");
        let parsed = ttf_parser::Face::parse(EMOJI, 0).expect("ttf-parser face");
        let mut checked = 0usize;
        for codepoint in 0u32..=0x10FFFF {
            let Some(ch) = char::from_u32(codepoint) else { continue };
            let Some(glyph_id) = parsed.glyph_index(ch) else { continue };
            assert!(
                face.image(glyph_id.0).is_some(),
                "U+{codepoint:04X} maps to glyph {} with no image",
                glyph_id.0
            );
            checked += 1;
        }
        assert!(checked > 200, "the shipped subset should carry hundreds of glyphs, got {checked}");
    }

    /// An out-of-range glyph is a miss, not a panic or a wrapped read.
    #[test]
    fn an_uncovered_glyph_is_a_miss() {
        let face = ColorBitmapFace::parse(EMOJI).expect("parse");
        assert!(face.image(0).is_none(), "glyph 0 is `.notdef` and has no image");
        assert!(face.image(u16::MAX).is_none());
    }

    /// A real emoji paints as **colour**, with more than one distinct RGB value.
    ///
    /// The distinct-colour count is what makes this a colour test rather than a coverage test: a
    /// 1-bit fallback would produce exactly one RGB triple (the ink colour) over the opaque pixels.
    #[test]
    fn a_real_emoji_paints_in_colour() {
        let face = ColorBitmapFace::parse(EMOJI).expect("parse");
        let parsed = ttf_parser::Face::parse(EMOJI, 0).expect("ttf-parser face");
        let cell = super::super::Cell::new(48, 48);
        let mut out = crate::compat::vec![0u8; cell.area() * 4];

        // The grinning face: multi-colour by construction (yellow skin, white eyes, red mouth).
        let glyph_id = parsed.glyph_index('\u{1F600}').expect("the grinning face is in the subset");
        face.paint(glyph_id.0, cell, &mut out).expect("the grinning face paints");

        let opaque = out.chunks_exact(4).filter(|px| px[3] != 0).count();
        assert!(opaque > 0, "some pixels must be opaque");
        let mut colours = std::collections::BTreeSet::new();
        for px in out.chunks_exact(4) {
            if px[3] != 0 {
                colours.insert((px[0], px[1], px[2]));
            }
        }
        assert!(
            colours.len() > 1,
            "a colour emoji must have more than one RGB value, got {}",
            colours.len()
        );
    }

    /// A cell the caller made too small for RGBA is refused rather than written past.
    #[test]
    fn a_cell_too_small_for_rgba_is_refused() {
        let face = ColorBitmapFace::parse(EMOJI).expect("parse");
        let parsed = ttf_parser::Face::parse(EMOJI, 0).expect("ttf-parser face");
        let glyph_id = parsed.glyph_index('\u{1F600}').expect("grinning face present");
        let cell = super::super::Cell::new(8, 8);
        // One byte per pixel: enough for a 1-bit glyph, not for colour.
        let mut out = crate::compat::vec![0u8; cell.area()];
        assert!(face.paint(glyph_id.0, cell, &mut out).is_none());
    }

    /// An empty cell is refused before any lookup.
    #[test]
    fn an_empty_cell_is_refused() {
        let face = ColorBitmapFace::parse(EMOJI).expect("parse");
        let parsed = ttf_parser::Face::parse(EMOJI, 0).expect("ttf-parser face");
        let glyph_id = parsed.glyph_index('\u{1F600}').expect("grinning face present");
        let mut out = crate::compat::vec![0u8; 4];
        assert!(face.paint(glyph_id.0, super::super::Cell::new(0, 0), &mut out).is_none());
    }

    /// A header length is defined for exactly the three PNG formats `CBDT` defines.
    #[test]
    fn only_the_png_image_formats_have_a_header_length() {
        assert_eq!(image_format_header_len(17), Some(9), "small metrics + dataLen");
        assert_eq!(image_format_header_len(18), Some(12), "big metrics + dataLen");
        assert_eq!(image_format_header_len(19), Some(4), "dataLen only");
        for legacy in [1u16, 2, 5, 6, 7, 8, 9, 20] {
            assert_eq!(image_format_header_len(legacy), None, "format {legacy} is not PNG");
        }
    }
}
