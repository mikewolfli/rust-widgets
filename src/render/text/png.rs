// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! A PNG decoder, written against the spec, for the colour-bitmap glyph path (G-6).
//!
//! # Why this is here rather than a dependency
//!
//! The crate already reaches for `image`/`resvg` behind its `image` feature, and `miniz_oxide` for
//! inflate. Neither is right for this call site: the text layer has to stay loadable on `mini` and
//! `embedded`, and pulling the whole `image` crate into the *text* path to decode a 109 px emoji
//! would be the kind of dependency the crate's own rules ask about. PNG's non-inflate part is about
//! 200 lines — unfilter five row types and expand five colour types — and it is fully specified, so
//! writing it keeps the dependency graph at the size the layer's constraints allow.
//!
//! # What is deliberately *not* supported, and why that is safe
//!
//! An unsupported input returns `None`. It never guesses. The cases:
//!
//! * **Interlaced (Adam7).** A `CBDT` emoji glyph is never interlaced — it is generated, not
//!   scanned — and supporting it would mean seven passes per glyph for a case nothing produces.
//!   `None`, and `tools/gen_emoji_subset.py` keeps the subsets non-interlaced.
//! * **Bit depths below 8 for colour types.** A `CBDT` glyph is 8-bit truecolour-with-alpha. The
//!   sub-byte expansions (1/2/4-bit greyscale, palette indices) exist in the spec and are refused
//!   here rather than half-implemented: a wrong expansion produces plausible-looking noise, which is
//!   the failure mode this module is most exposed to.
//! * **`sBIT`, `gAMA`, `iCCP`, interlacing chunk ordering.** All advisory for this use.
//!
//! # Colour type coverage, which is the part that actually matters
//!
//! | type | meaning | supported |
//! |---|---|---|
//! | 0 | greyscale | yes, 8/16 bit |
//! | 2 | truecolour | yes, 8/16 bit |
//! | 3 | palette | yes, 8-bit indices |
//! | 4 | greyscale + alpha | yes, 8/16 bit |
//! | 6 | truecolour + alpha | yes, 8/16 bit |
//!
//! Everything is emitted as straight (non-premultiplied) RGBA8, which is what the render path
//! blends.

/// A decoded image: straight RGBA8, row-major.
pub struct Decoded<'a> {
    /// Pixel width.
    pub width: u32,
    /// Pixel height.
    pub height: u32,
    /// `width * height * 4` bytes of RGBA.
    pub pixels: &'a [u8],
}

/// The eight-byte PNG signature.
const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];

/// Decodes `data` into `out`, which must hold at least `width * height * 4` bytes.
///
/// Returns `None` for anything this decoder does not support — see the module docs. The returned
/// `Decoded` borrows `out`, so the caller keeps ownership of the buffer it already has.
pub fn decode_png<'a>(data: &[u8], out: &'a mut [u8]) -> Option<Decoded<'a>> {
    if !data.starts_with(&SIGNATURE) {
        return None;
    }
    let mut reader = Chunks::new(data, SIGNATURE.len());
    let mut header = None;
    let mut palette = None;
    let mut transparency = None;
    let mut idat = crate::compat::Vec::new();

    while let Some(chunk) = reader.next_chunk()? {
        match &chunk.kind {
            b"IHDR" => header = Some(parse_ihdr(chunk.data)?),
            b"PLTE" => palette = Some(chunk.data),
            b"tRNS" => transparency = Some(chunk.data),
            b"IDAT" => idat.extend_from_slice(chunk.data),
            b"IEND" => break,
            // Every other chunk is advisory for this use, and an unknown *critical* chunk would
            // have to be refused — but the ones a CBDT glyph carries (`bKGD`, `pHYs`, `tEXt`,
            // `sRGB`, `gAMA`, `sBIT`, `iCCP`, `tIME`) are all ancillary, i.e. their first letter is
            // lowercase.
            _ => {
                let critical = chunk.kind[0].is_ascii_uppercase();
                if critical {
                    return None;
                }
            }
        }
    }

    let header = header?;
    if header.interlaced {
        return None;
    }
    let channels = channels_for(header.color_type)?;
    let bytes_per_sample = match header.bit_depth {
        8 => 1usize,
        16 => 2usize,
        // Sub-byte depths are only meaningful for types 0 and 3, and expanding them is the part of
        // the spec most likely to be got subtly wrong; see the module docs.
        _ => return None,
    };
    // A palette image must carry its palette.
    if header.color_type == 3 && palette.is_none() {
        return None;
    }

    let row_bytes = (header.width as usize).checked_mul(channels)?.checked_mul(bytes_per_sample)?;
    let expected = header.height as usize * row_bytes;
    let mut raw = crate::compat::vec![0u8; expected];
    inflate_into(&idat, &mut raw)?;

    // Unfilter in place, one row at a time, using the previous row as the reference.
    unfilter(&mut raw, header.height as usize, row_bytes, bytes_per_sample * channels)?;

    let needed = (header.width as usize).checked_mul(header.height as usize)?.checked_mul(4)?;
    if out.len() < needed {
        return None;
    }
    expand(&raw, header, channels, bytes_per_sample, palette, transparency, &mut out[..needed])?;

    Some(Decoded { width: header.width, height: header.height, pixels: &out[..needed] })
}

/// The `IHDR` fields this decoder uses.
#[derive(Debug, Clone, Copy)]
struct Ihdr {
    width: u32,
    height: u32,
    bit_depth: u8,
    color_type: u8,
    interlaced: bool,
}

/// Reads `IHDR`: 13 bytes, `width`, `height`, `bitDepth`, `colorType`, `compression`, `filter`,
/// `interlace`.
fn parse_ihdr(data: &[u8]) -> Option<Ihdr> {
    if data.len() < 13 {
        return None;
    }
    let width = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let height = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if width == 0 || height == 0 {
        return None;
    }
    let bit_depth = data[8];
    let color_type = data[9];
    // Only deflate and the standard filter set are defined; anything else is a future PNG this
    // decoder must not guess about.
    if data[10] != 0 || data[11] != 0 {
        return None;
    }
    let interlace = match data[12] {
        0 => false,
        1 => true,
        _ => return None,
    };
    Some(Ihdr { width, height, bit_depth, color_type, interlaced: interlace })
}

/// Channels per pixel for a colour type.
fn channels_for(color_type: u8) -> Option<usize> {
    match color_type {
        0 => Some(1), // greyscale
        2 => Some(3), // truecolour
        3 => Some(1), // palette index
        4 => Some(2), // greyscale + alpha
        6 => Some(4), // truecolour + alpha
        _ => None,
    }
}

/// A chunk iterator that validates each chunk's CRC.
struct Chunks<'a> {
    data: &'a [u8],
    offset: usize,
}

/// One chunk, borrowed from the file.
struct Chunk<'a> {
    kind: [u8; 4],
    data: &'a [u8],
}

impl<'a> Chunks<'a> {
    const fn new(data: &'a [u8], offset: usize) -> Self {
        Self { data, offset }
    }

    /// The next chunk, or `None` at the end or on a malformed one.
    fn next_chunk(&mut self) -> Option<Option<Chunk<'a>>> {
        let header = self.data.get(self.offset..self.offset + 8)?;
        let length = u32::from_be_bytes([header[0], header[1], header[2], header[3]]) as usize;
        let kind = [header[4], header[5], header[6], header[7]];
        let start = self.offset + 8;
        let payload = self.data.get(start..start.checked_add(length)?)?;
        let crc_bytes = self.data.get(start + length..start + length + 4)?;
        let expected = u32::from_be_bytes([crc_bytes[0], crc_bytes[1], crc_bytes[2], crc_bytes[3]]);
        // The CRC covers the type and the payload, and checking it is what turns a truncated or
        // corrupted glyph into a miss instead of a picture assembled from garbage.
        if crc32(&kind, payload) != expected {
            return None;
        }
        self.offset = start + length + 4;
        Some(Some(Chunk { kind, data: payload }))
    }
}

/// CRC-32 (IEEE 802.3), as PNG specifies.
fn crc32(kind: &[u8; 4], payload: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for byte in kind.iter().chain(payload.iter()) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

/// Inflates `data` into `out`, requiring exactly `out.len()` bytes.
///
/// `miniz_oxide` is the crate's existing inflate dependency, so this adds nothing new.
fn inflate_into(data: &[u8], out: &mut [u8]) -> Option<()> {
    use miniz_oxide::inflate::core::{decompress, inflate_flags};
    use miniz_oxide::inflate::TINFLStatus;

    let mut decompressor = miniz_oxide::inflate::core::DecompressorOxide::new();
    let flags = inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER;
    let (status, _consumed, produced) = decompress(&mut decompressor, data, out, 0, flags);
    if status != TINFLStatus::Done && status != TINFLStatus::HasMoreOutput {
        return None;
    }
    // A short output means the stream was truncated; a long one is impossible to observe here
    // because `decompress` stops at `out`'s end. Requiring the full length is what rejects both.
    if produced != out.len() {
        return None;
    }
    Some(())
}

/// Reverses PNG's per-row filters, in place.
///
/// # The five filter types
///
/// | type | name | reference |
/// |---|---|---|
/// | 0 | None | nothing |
/// | 1 | Sub | the byte one pixel to the left |
/// | 2 | Up | the byte directly above |
/// | 3 | Average | the mean of left and above |
/// | 4 | Paeth | a predictor choosing among left, above and their corner |
///
/// Each row's first byte is its filter type; `stride` is `bpp` for Sub and Average, because the
/// reference pixel is one *pixel* to the left, not one byte.
fn unfilter(raw: &mut [u8], height: usize, row_bytes: usize, bpp: usize) -> Option<()> {
    let stride = row_bytes + 1;
    if raw.len() < stride * height {
        return None;
    }
    // A row's reference is the row **before it**, and the unfiltering is in place. That is a
    // read-then-write on adjacent slices, which the borrow checker cannot see as disjoint when both
    // are taken from the same buffer. The previous row is therefore copied into a small scratch
    // first: one row of scratch, not one image, so the memory cost is a row rather than a frame.
    let mut previous = crate::compat::vec![0u8; row_bytes];
    for row in 0..height {
        let start = row * stride;
        let filter = *raw.get(start)?;
        let row_start = start + 1;
        let row_slice = raw.get_mut(row_start..row_start + row_bytes)?;
        // The row's own pre-unfilter bytes, needed as the next iteration's `previous`.
        let mut current = previous.clone();
        current.copy_from_slice(row_slice);
        for i in 0..row_bytes {
            let left = if i >= bpp { row_slice[i - bpp] } else { 0 };
            let above = previous.get(i).copied().unwrap_or(0);
            let corner = if i >= bpp { previous.get(i - bpp).copied().unwrap_or(0) } else { 0 };
            let value = row_slice[i];
            row_slice[i] = match filter {
                0 => value,
                1 => value.wrapping_add(left),
                2 => value.wrapping_add(above),
                3 => value.wrapping_add(((u16::from(left) + u16::from(above)) / 2) as u8),
                4 => value.wrapping_add(paeth(left, above, corner)),
                // An unknown filter type cannot be reversed, and guessing would produce noise.
                _ => return None,
            };
        }
        previous.copy_from_slice(&current);
        // The scratch holds the *unfiltered* row now, which is what the next row needs as its
        // `above`. Re-read the just-written bytes rather than tracking them separately — one copy of
        // a row, and no second buffer whose contents could diverge from the buffer's.
        previous.copy_from_slice(raw.get(row_start..row_start + row_bytes)?);
    }
    Some(())
}

/// The Paeth predictor: pick whichever of left/above/corner is closest to `left + above - corner`.
fn paeth(left: u8, above: u8, corner: u8) -> u8 {
    let p = i16::from(left) + i16::from(above) - i16::from(corner);
    let pa = (p - i16::from(left)).abs();
    let pb = (p - i16::from(above)).abs();
    let pc = (p - i16::from(corner)).abs();
    if pa <= pb && pa <= pc {
        left
    } else if pb <= pc {
        above
    } else {
        corner
    }
}

/// Expands unfiltered samples into straight RGBA8.
fn expand(
    raw: &[u8],
    header: Ihdr,
    channels: usize,
    bytes_per_sample: usize,
    palette: Option<&[u8]>,
    transparency: Option<&[u8]>,
    out: &mut [u8],
) -> Option<()> {
    let width = header.width as usize;
    let height = header.height as usize;
    let row_bytes = width.checked_mul(channels)?.checked_mul(bytes_per_sample)?;
    let stride = row_bytes + 1;
    let max = if header.bit_depth == 16 { 65535u32 } else { 255u32 };

    // A 16-bit sample is reduced to 8 by keeping the high byte, which is the standard reduction.
    let sample = |row: &[u8], index: usize| -> u8 {
        let byte_index = index * bytes_per_sample;
        if bytes_per_sample == 2 {
            row.get(byte_index).copied().unwrap_or(0)
        } else {
            row.get(byte_index).copied().unwrap_or(0)
        }
    };

    for y in 0..height {
        let row = raw.get(y * stride + 1..(y + 1) * stride)?;
        for x in 0..width {
            let di = (y * width + x) * 4;
            let target = out.get_mut(di..di + 4)?;
            let base = x * channels;
            match header.color_type {
                0 => {
                    let g = scale_to_byte(sample(row, base), max);
                    // `tRNS` for greyscale is one 16-bit sample naming the transparent value.
                    let alpha = match transparency {
                        Some(t) if t.len() >= 2 => {
                            let key = u16::from_be_bytes([t[0], t[1]]);
                            if u16::from(sample_full(row, base, bytes_per_sample)) == key {
                                0
                            } else {
                                255
                            }
                        }
                        _ => 255,
                    };
                    target.copy_from_slice(&[g, g, g, alpha]);
                }
                2 => {
                    let r = scale_to_byte(sample(row, base), max);
                    let g = scale_to_byte(sample(row, base + 1), max);
                    let b = scale_to_byte(sample(row, base + 2), max);
                    let alpha = match transparency {
                        Some(t) if t.len() >= 6 => {
                            let kr = u16::from_be_bytes([t[0], t[1]]);
                            let kg = u16::from_be_bytes([t[2], t[3]]);
                            let kb = u16::from_be_bytes([t[4], t[5]]);
                            let (r16, g16, b16) = (
                                u16::from(sample_full(row, base, bytes_per_sample)),
                                u16::from(sample_full(row, base + 1, bytes_per_sample)),
                                u16::from(sample_full(row, base + 2, bytes_per_sample)),
                            );
                            if (r16, g16, b16) == (kr, kg, kb) {
                                0
                            } else {
                                255
                            }
                        }
                        _ => 255,
                    };
                    target.copy_from_slice(&[r, g, b, alpha]);
                }
                3 => {
                    let index = sample(row, base) as usize;
                    let entry = palette?.get(index * 3..index * 3 + 3)?;
                    let alpha = transparency.and_then(|t| t.get(index).copied()).unwrap_or(255);
                    target.copy_from_slice(&[entry[0], entry[1], entry[2], alpha]);
                }
                4 => {
                    let g = scale_to_byte(sample(row, base), max);
                    let a = scale_to_byte(sample(row, base + 1), max);
                    target.copy_from_slice(&[g, g, g, a]);
                }
                6 => {
                    let r = scale_to_byte(sample(row, base), max);
                    let g = scale_to_byte(sample(row, base + 1), max);
                    let b = scale_to_byte(sample(row, base + 2), max);
                    let a = scale_to_byte(sample(row, base + 3), max);
                    target.copy_from_slice(&[r, g, b, a]);
                }
                _ => return None,
            }
        }
    }
    Some(())
}

/// One sample widened to `u16`, so a `tRNS` key can be compared at the file's own depth.
fn sample_full(row: &[u8], index: usize, bytes_per_sample: usize) -> u16 {
    if bytes_per_sample == 2 {
        let i = index * 2;
        let hi = row.get(i).copied().unwrap_or(0);
        let lo = row.get(i + 1).copied().unwrap_or(0);
        u16::from_be_bytes([hi, lo])
    } else {
        u16::from(row.get(index).copied().unwrap_or(0))
    }
}

/// Reduces a sample to 8 bits by its declared maximum.
fn scale_to_byte(value: u8, max: u32) -> u8 {
    if max == 255 {
        value
    } else {
        // A 16-bit value has already been reduced to its high byte, so it is already in `0..=255`.
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 1x1 opaque red PNG, byte-exact.
    ///
    /// Hand-built rather than generated so the test does not depend on an encoder. The deflate
    /// stream is a stored (uncompressed) block, which is legal deflate and the one form whose bytes
    /// can be written out by a person.
    fn red_pixel_png() -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(&SIGNATURE);
        // IHDR: 1x1, 8-bit, truecolour+alpha.
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
        push_chunk(&mut png, b"IHDR", &ihdr);
        // IDAT: zlib{ deflate stored block: filter byte 0 + RGBA (255,0,0,255) } adler32.
        let mut raw = Vec::new();
        raw.push(0); // filter: none
        raw.extend_from_slice(&[255, 0, 0, 255]);
        let mut deflate = Vec::new();
        deflate.push(0x01); // final stored block
        let len = raw.len() as u16;
        deflate.extend_from_slice(&len.to_le_bytes());
        deflate.extend_from_slice(&(!len).to_le_bytes());
        deflate.extend_from_slice(&raw);
        let mut zlib = Vec::new();
        zlib.push(0x78);
        zlib.push(0x01);
        zlib.extend_from_slice(&deflate);
        zlib.extend_from_slice(&adler32(&raw).to_be_bytes());
        push_chunk(&mut png, b"IDAT", &zlib);
        push_chunk(&mut png, b"IEND", &[]);
        png
    }

    fn push_chunk(out: &mut Vec<u8>, kind: &[u8; 4], payload: &[u8]) {
        out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
        out.extend_from_slice(kind);
        out.extend_from_slice(payload);
        out.extend_from_slice(&crc32(kind, payload).to_be_bytes());
    }

    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for byte in data {
            a = (a + u32::from(*byte)) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    #[test]
    fn a_truecolour_alpha_pixel_decodes() {
        let png = red_pixel_png();
        let mut out = vec![0u8; 4];
        let decoded = decode_png(&png, &mut out).expect("a valid PNG decodes");
        assert_eq!((decoded.width, decoded.height), (1, 1));
        assert_eq!(decoded.pixels, &[255, 0, 0, 255]);
    }

    /// A corrupted payload is refused, not decoded into noise.
    ///
    /// The CRC is the only thing standing between a truncated download and a picture assembled from
    /// whatever was in the buffer, so it gets its own test.
    #[test]
    fn a_bad_crc_is_refused() {
        let mut png = red_pixel_png();
        // Flip a byte inside the IDAT payload.
        let len = png.len();
        png[len - 10] ^= 0xff;
        let mut out = vec![0u8; 4];
        assert!(decode_png(&png, &mut out).is_none(), "a corrupted chunk must be refused");
    }

    /// A non-PNG is refused at the signature.
    #[test]
    fn a_wrong_signature_is_refused() {
        let mut out = vec![0u8; 4];
        assert!(decode_png(b"not a png at all", &mut out).is_none());
        assert!(decode_png(&[], &mut out).is_none());
    }

    /// A buffer too small for the image is refused.
    #[test]
    fn a_short_buffer_is_refused() {
        let png = red_pixel_png();
        let mut out = vec![0u8; 3];
        assert!(decode_png(&png, &mut out).is_none());
    }

    /// The Paeth predictor picks the closest of its three candidates.
    #[test]
    fn the_paeth_predictor_is_symmetric() {
        assert_eq!(paeth(10, 20, 30), 20);
        assert_eq!(paeth(0, 0, 0), 0);
        assert_eq!(paeth(255, 0, 0), 255);
    }

    /// The colour-type table is total over the five spec values.
    #[test]
    fn colour_types_map_to_channel_counts() {
        assert_eq!(channels_for(0), Some(1));
        assert_eq!(channels_for(2), Some(3));
        assert_eq!(channels_for(3), Some(1));
        assert_eq!(channels_for(4), Some(2));
        assert_eq!(channels_for(6), Some(4));
        assert_eq!(channels_for(1), None, "1 is not a PNG colour type");
        assert_eq!(channels_for(5), None);
    }
}
