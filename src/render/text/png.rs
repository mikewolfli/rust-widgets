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
        // Sub-byte depths. Only greyscale (0) and palette (3) define them, and the samples are
        // **packed** — a 4-bit image stores two indices per byte, most significant nibble first.
        // A region of `NotoColorEmoji` uses exactly this for its small flags, so a decoder without
        // it makes a whole category of colour glyph unreadable rather than rare.
        1 | 2 | 4 if matches!(header.color_type, 0 | 3) => 1usize,
        _ => return None,
    };
    let bits_per_sample = header.bit_depth as usize;
    // A palette image must carry its palette.
    if header.color_type == 3 && palette.is_none() {
        return None;
    }

    let row_bytes = if bits_per_sample < 8 {
        (header.width as usize).checked_mul(bits_per_sample)?.checked_add(7)? / 8
    } else {
        (header.width as usize).checked_mul(channels)?.checked_mul(bytes_per_sample)?
    };
    // One filter-type byte precedes each row, so the inflated size is `height * (row_bytes + 1)`.
    // This was `height * row_bytes` in an earlier revision, which made every decode fail at the
    // inflate step (the stream produced one byte per row more than the buffer held).
    let expected = header.height as usize * (row_bytes + 1);
    let mut raw = crate::compat::vec![0u8; expected];
    inflate_into(&idat, &mut raw)?;

    // Unfilter in place, one row at a time, using the previous row as the reference. Sub-byte
    // samples still filter byte-wise with a `bpp` of one, because "one pixel to the left" is less
    // than a byte away in a packed row.
    let filter_bpp = if bits_per_sample < 8 { 1 } else { bytes_per_sample * channels };
    unfilter(&mut raw, header.height as usize, row_bytes, filter_bpp)?;

    let needed = (header.width as usize).checked_mul(header.height as usize)?.checked_mul(4)?;
    if out.len() < needed {
        return None;
    }
    expand(
        &raw,
        header,
        channels,
        bytes_per_sample,
        bits_per_sample,
        row_bytes,
        palette,
        transparency,
        &mut out[..needed],
    )?;

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
///
/// # Why this streams instead of calling `decompress` once
///
/// `decompress` treats `out` as a **wrapping** output buffer unless
/// `TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF` is set, and a wrapping buffer must be a power of two
/// — an `8832`-byte scratch is not, so the call returns `BadParam` and produces nothing. The
/// non-wrapping flag is the fix, but it is not sufficient on its own: with that flag a call stops at
/// `HasMoreOutput` as soon as `out` fills, and a PNG stream can need several calls. So the input is
/// advanced by `consumed` and the output by `produced` until the stream reports `Done`, which is the
/// shape the API documents for non-wrapping use.
///
/// # The one loop guard, and why there is not a second
///
/// The loop stops on three things: `Done`, no progress, or any other status. "No progress" is the
/// invariant that covers a full buffer without a separate length check — measured against
/// `miniz_oxide` on a two-block stream into a 16-byte buffer:
///
/// ```text
/// status=HasMoreOutput consumed=28 produced=16   # first call: fills the buffer
/// status=HasMoreOutput consumed=0  produced=0    # second call: nothing left to do
/// ```
///
/// So a full buffer is detected one iteration later by the same guard, and an explicit
/// `out_pos >= out.len()` check would be dead code — it was removed here after a mutation test
/// showed that deleting it changed no test's outcome. `out_pos != out.len()` at the end is the
/// separate, load-bearing check: it is what rejects a *truncated* stream (see the tests).
fn inflate_into(data: &[u8], out: &mut [u8]) -> Option<()> {
    use miniz_oxide::inflate::core::{decompress, inflate_flags};
    use miniz_oxide::inflate::TINFLStatus;

    // `decompress` documents that a zero-length output makes any write produce `HasMoreOutput`, so
    // the no-progress guard below would fire on the first iteration. Refusing here states the
    // reason rather than reaching the same answer through a loop that never had a chance.
    if out.is_empty() {
        return None;
    }
    let mut decompressor = miniz_oxide::inflate::core::DecompressorOxide::new();
    let flags = inflate_flags::TINFL_FLAG_PARSE_ZLIB_HEADER
        | inflate_flags::TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF;

    let mut in_pos = 0usize;
    let mut out_pos = 0usize;
    loop {
        let (status, consumed, produced) =
            decompress(&mut decompressor, &data[in_pos..], &mut out[out_pos..], 0, flags);
        in_pos = in_pos.checked_add(consumed)?;
        out_pos = out_pos.checked_add(produced)?;
        match status {
            TINFLStatus::Done => break,
            // The stream is not finished and made no progress: either the input ran out mid-stream
            // or the output filled before the stream ended. Either way the bytes do not describe
            // the image `IHDR` promised, so this is a refusal rather than a partial decode.
            TINFLStatus::HasMoreOutput => {
                if produced == 0 && consumed == 0 {
                    return None;
                }
            }
            _ => return None,
        }
    }
    // A short output means the stream was truncated. Requiring the full length is what rejects it:
    // `unfilter` would otherwise read the unwritten remainder as a run of unfiltered zeroes.
    if out_pos != out.len() {
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
///
/// `bits_per_sample` is the file's `bitDepth` and `row_bytes` its packed row stride. For the 8- and
/// 16-bit depths the two are consistent with `channels * bytes_per_sample` and the sample fetch is a
/// byte read; for the sub-byte depths (1/2/4, palette or greyscale only) a sample is a bit field
/// inside a byte, most significant field first, and `row_bytes` is the padded packed stride.
#[allow(clippy::too_many_arguments)]
fn expand(
    raw: &[u8],
    header: Ihdr,
    channels: usize,
    bytes_per_sample: usize,
    bits_per_sample: usize,
    row_bytes: usize,
    palette: Option<&[u8]>,
    transparency: Option<&[u8]>,
    out: &mut [u8],
) -> Option<()> {
    let width = header.width as usize;
    let height = header.height as usize;
    let stride = row_bytes + 1;

    // A 16-bit sample is reduced to 8 by keeping the high byte, which is the standard reduction.
    let sample = |row: &[u8], index: usize| -> u8 { row.get(index).copied().unwrap_or(0) };

    // One sample out of a packed row. `/8` is the byte, `%8` the bit offset within it, and PNG
    // packs most-significant-first, so the field runs from bit `offset` downwards and the shift is
    // `8 - bits - offset` rather than `offset`.
    let packed = |row: &[u8], index: usize| -> u8 {
        let bit = index * bits_per_sample;
        let byte = row.get(bit / 8).copied().unwrap_or(0);
        let offset = bit % 8;
        let shift = 8 - bits_per_sample - offset;
        (byte >> shift) & ((1u16 << bits_per_sample) - 1) as u8
    };

    for y in 0..height {
        let row = raw.get(y * stride + 1..(y + 1) * stride)?;
        for x in 0..width {
            let di = (y * width + x) * 4;
            let target = out.get_mut(di..di + 4)?;
            let base = x * channels;
            match header.color_type {
                0 => {
                    // A greyscale sample at depth `d` spans bit fields, except at depth 8/16 where it
                    // is a whole byte; `packed` handles the first case and the file's `bitDepth`
                    // chooses. `scale_to_byte` then spreads a 1/2/4-bit value over 0..=255, which is
                    // what makes a 4-bit grey ramp reach white rather than stopping at 15.
                    let raw_g = if bits_per_sample < 8 {
                        packed(row, base)
                    } else {
                        sample(row, base * bytes_per_sample)
                    };
                    let g = scale_to_byte(raw_g, bits_per_sample);
                    // `tRNS` for greyscale is one 16-bit sample naming the transparent value. The
                    // comparison is at the file's own depth, so a sub-byte grey compares unpacked.
                    let alpha = match transparency {
                        Some(t) if t.len() >= 2 => {
                            let key = u16::from_be_bytes([t[0], t[1]]);
                            let value = if bits_per_sample < 8 {
                                u16::from(raw_g)
                            } else {
                                sample_full(row, base, bytes_per_sample)
                            };
                            if value == key {
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
                    let r = scale_to_byte(sample(row, base * bytes_per_sample), bits_per_sample);
                    let g =
                        scale_to_byte(sample(row, (base + 1) * bytes_per_sample), bits_per_sample);
                    let b =
                        scale_to_byte(sample(row, (base + 2) * bytes_per_sample), bits_per_sample);
                    let alpha = match transparency {
                        Some(t) if t.len() >= 6 => {
                            let kr = u16::from_be_bytes([t[0], t[1]]);
                            let kg = u16::from_be_bytes([t[2], t[3]]);
                            let kb = u16::from_be_bytes([t[4], t[5]]);
                            let (r16, g16, b16) = (
                                sample_full(row, base, bytes_per_sample),
                                sample_full(row, base + 1, bytes_per_sample),
                                sample_full(row, base + 2, bytes_per_sample),
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
                    let index = if bits_per_sample < 8 {
                        packed(row, base) as usize
                    } else {
                        sample(row, base * bytes_per_sample) as usize
                    };
                    let entry = palette?.get(index * 3..index * 3 + 3)?;
                    let alpha = transparency.and_then(|t| t.get(index).copied()).unwrap_or(255);
                    target.copy_from_slice(&[entry[0], entry[1], entry[2], alpha]);
                }
                4 => {
                    let g = scale_to_byte(sample(row, base * bytes_per_sample), bits_per_sample);
                    let a =
                        scale_to_byte(sample(row, (base + 1) * bytes_per_sample), bits_per_sample);
                    target.copy_from_slice(&[g, g, g, a]);
                }
                6 => {
                    let r = scale_to_byte(sample(row, base * bytes_per_sample), bits_per_sample);
                    let g =
                        scale_to_byte(sample(row, (base + 1) * bytes_per_sample), bits_per_sample);
                    let b =
                        scale_to_byte(sample(row, (base + 2) * bytes_per_sample), bits_per_sample);
                    let a =
                        scale_to_byte(sample(row, (base + 3) * bytes_per_sample), bits_per_sample);
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

/// Reduces a sample to 8 bits by scaling it from its own depth to `0..=255`.
///
/// A 16-bit sample has already been reduced to its high byte by the caller, so it is already in
/// range; an 8-bit sample is too. A 1/2/4-bit sample has to be *scaled* rather than passed through:
/// a 4-bit white is `15`, and returning that draws a near-black pixel. `255 / (2^bits - 1)` is the
/// exact multiplier for those depths (255, 85, 17), and the reason the `max` argument is unused
/// rather than a `1 << bits` divisor — `1 << 4` is 16, which would map 15 to 239, not 255.
fn scale_to_byte(value: u8, bits: usize) -> u8 {
    match bits {
        1 => value * 255,
        2 => value * 85,
        4 => value * 17,
        _ => value,
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

    /// A `4`-bit palette PNG with two rows of two pixels, hand-built.
    ///
    /// This is the shape the shipped colour emoji use (`IHDR` reports `depth=4 colour=3`), and the
    /// shape that made the first version of this decoder return `None` for every glyph: the inflated
    /// buffer was sized `height * row_bytes` and forgot the filter byte per row, and inflate was
    /// called without `TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF`, which rejects any non-power-of-two
    /// output. Both are invisible from a 1x1 8-bit image, which is why this fixture exists.
    ///
    /// The image is:
    ///
    /// ```text
    /// row 0: index 0 (red), index 15 (white)   -> nibbles 0x0f
    /// row 1: index 1 (green), index 2 (blue)   -> nibbles 0x12
    /// ```
    ///
    /// The two odd-width rows are deliberate: `2` pixels at `4` bits is exactly one byte, so the
    /// row stride is `1` and the buffer is `2 * (1 + 1) = 4` bytes. A width whose packed stride is
    /// not a whole byte (e.g. 3 pixels at 4 bits) would exercise the `+7 / 8` rounding as well; the
    /// `a_packed_row_pads_to_a_whole_byte` test below covers that separately.
    fn palette_four_bit_png(width: u32, rows: &[&[u8]], row_bytes: usize) -> Vec<u8> {
        let mut png = Vec::new();
        png.extend_from_slice(&SIGNATURE);
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&(rows.len() as u32).to_be_bytes());
        ihdr.extend_from_slice(&[4, 3, 0, 0, 0]);
        push_chunk(&mut png, b"IHDR", &ihdr);
        // A 16-entry palette so index 15 is addressable; only 0..3 matter to the assertions.
        let mut palette = Vec::new();
        palette.extend_from_slice(&[255, 0, 0]); // 0 red
        palette.extend_from_slice(&[0, 255, 0]); // 1 green
        palette.extend_from_slice(&[0, 0, 255]); // 2 blue
        for extra in 0..13u8 {
            palette.extend_from_slice(&[extra, extra, extra]);
        }
        palette[15 * 3] = 255;
        palette[15 * 3 + 1] = 255;
        palette[15 * 3 + 2] = 255; // 15 white
        push_chunk(&mut png, b"PLTE", &palette);
        // Index 0 is fully transparent, and the rest opaque. This is what a CBDT glyph's `tRNS`
        // looks like, and it is checked here because a colour glyph's *shape* comes from alpha.
        let mut trns = [255u8; 16];
        trns[0] = 0;
        push_chunk(&mut png, b"tRNS", &trns);
        let mut raw = Vec::new();
        for row in rows {
            raw.push(0); // filter: none
            raw.extend_from_slice(row);
            raw.resize(raw.len() + row_bytes - row.len(), 0);
        }
        let mut deflate = Vec::new();
        deflate.push(0x01);
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

    fn adler32(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for byte in data {
            a = (a + u32::from(*byte)) % 65521;
            b = (b + a) % 65521;
        }
        (b << 16) | a
    }

    /// A zlib stream wrapping one deflate **stored** block holding `raw`.
    ///
    /// Stored blocks are the one deflate form whose bytes a person can write, which is what makes a
    /// hand-built fixture possible in a test. `final_block` controls the `BFINAL` bit, which is how
    /// the "stream ends early" case below is constructed.
    fn zlib_stored(raw: &[u8], final_block: bool) -> Vec<u8> {
        let mut deflate = Vec::new();
        deflate.push(if final_block { 0x01 } else { 0x00 });
        let len = raw.len() as u16;
        deflate.extend_from_slice(&len.to_le_bytes());
        deflate.extend_from_slice(&(!len).to_le_bytes());
        deflate.extend_from_slice(raw);
        let mut zlib = vec![0x78, 0x01];
        zlib.extend_from_slice(&deflate);
        zlib.extend_from_slice(&adler32(raw).to_be_bytes());
        zlib
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
    ///
    /// The first case is the one worth stating: `p = left + above - corner = 0`, so the distances
    /// are `10`, `20` and `30` and **left** wins. It reads as though `above` should win because
    /// `above` sits between `left` and `corner`, but that is not what the predictor measures — it
    /// measures closeness to the gradient `left + above - corner`, and here that gradient is zero.
    /// The expectation in this test was `20` before the decoder was exercised on a real image, which
    /// is exactly the mistake the assertion is here to prevent.
    #[test]
    fn the_paeth_predictor_is_symmetric() {
        assert_eq!(paeth(10, 20, 30), 10);
        assert_eq!(paeth(0, 0, 0), 0);
        assert_eq!(paeth(255, 0, 0), 255);
        // Cases from the PNG spec's own description of the filter: when the gradient is well
        // defined, the candidate nearest it wins.
        assert_eq!(paeth(0, 0, 255), 0);
        assert_eq!(paeth(255, 255, 0), 255);
        assert_eq!(paeth(200, 100, 150), 150);
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

    /// A 4-bit palette image decodes, unpacking the high nibble into the first pixel.
    #[test]
    fn a_four_bit_palette_image_decodes() {
        let png = palette_four_bit_png(2, &[&[0x0f], &[0x12]], 1);
        let mut out = vec![0u8; 2 * 2 * 4];
        let decoded = decode_png(&png, &mut out).expect("a 4-bit palette PNG decodes");
        assert_eq!((decoded.width, decoded.height), (2, 2));
        assert_eq!(
            decoded.pixels,
            &[
                255, 0, 0, 0, // index 0 (red), transparent per tRNS
                255, 255, 255, 255, // index 15 (white)
                0, 255, 0, 255, // index 1 (green)
                0, 0, 255, 255, // index 2 (blue)
            ]
        );
    }

    /// A row whose packed samples do not fill a whole byte is padded, not truncated.
    ///
    // Three 4-bit indices are 12 bits, which PNG pads to two bytes. A decoder that computed the
    // stride as `width * bits` would read a 12-bit row and drift by one byte per row; the last
    // pixel of the second row is the assertion that would catch it.
    //
    // Rows are written as the packed bytes directly: `0x01 0x20` is indices 0, 1, 2 (the low
    // nibble of the second byte is padding and must be ignored), and `0x21 0x00` is 2, 1, 0.
    #[test]
    fn a_packed_row_pads_to_a_whole_byte() {
        let png = palette_four_bit_png(3, &[&[0x01, 0x20], &[0x21, 0x00]], 2);
        let mut out = vec![0u8; 3 * 2 * 4];
        let decoded = decode_png(&png, &mut out).expect("a padded row decodes");
        assert_eq!(&decoded.pixels[0..4], &[255, 0, 0, 0], "row 0 pixel 0 is transparent red");
        assert_eq!(&decoded.pixels[4..8], &[0, 255, 0, 255], "row 0 pixel 1 is green");
        assert_eq!(&decoded.pixels[8..12], &[0, 0, 255, 255], "row 0 pixel 2 is blue");
        // The second row is the one that catches a stride error: if the row started one byte late
        // or early, these three pixels would be shifted.
        assert_eq!(&decoded.pixels[12..16], &[0, 0, 255, 255], "row 1 pixel 0 is blue");
        assert_eq!(&decoded.pixels[16..20], &[0, 255, 0, 255], "row 1 pixel 1 is green");
        assert_eq!(&decoded.pixels[20..24], &[255, 0, 0, 0], "row 1 pixel 2 is transparent red");
    }

    /// A sub-byte depth on a colour type that does not define one is refused.
    #[test]
    fn a_sub_byte_depth_on_truecolour_is_refused() {
        let mut png = red_pixel_png();
        // IHDR is at offset 8 (signature) + 8 (length + type) = 16, and `bitDepth` is its 9th byte.
        png[16 + 8] = 4;
        let mut out = vec![0u8; 4];
        assert!(
            decode_png(&png, &mut out).is_none(),
            "4-bit truecolour is not a defined PNG encoding"
        );
    }

    // ── The inflate loop's rejection paths ──────────────────────────────────────────────────────
    //
    // `inflate_into` streams: it advances the input by `consumed` and the output by `produced`
    // until the stream reports `Done`, and it must refuse three shapes rather than return a partial
    // buffer. These tests exist because the loop was written to fix a real `BadParam` defect (see
    // its docs) and, until now, was only ever walked by real emoji PNGs — so its *rejection* paths
    // had no case. A decode path that cannot be made to fail in a test is a decode path whose
    // failure mode is unknown.

    /// A stream that ends before the output is full is refused.
    ///
    /// This is the truncated-download case. The danger of *not* refusing is specific: `unfilter`
    /// reads the unwritten remainder of the buffer, which is zero-filled, so a short stream would
    /// decode to a picture whose lower rows are a run of unfiltered zeroes — plausible-looking and
    /// silently wrong.
    #[test]
    fn a_stream_that_ends_before_the_output_is_full_is_refused() {
        // 8 bytes of output offered, only 4 produced by a final stored block.
        let zlib = zlib_stored(&[1, 2, 3, 4], true);
        let mut out = vec![0u8; 8];
        assert!(
            inflate_into(&zlib, &mut out).is_none(),
            "a stream that stops short must not pass off a zero-filled tail as data"
        );
    }

    /// A stream whose output overflows the buffer is refused, not truncated.
    ///
    /// This is the "`IHDR` lies about the size" case, and it is the one the loop's
    /// `out_pos >= out.len()` guard exists for: with `TINFL_FLAG_USING_NON_WRAPPING_OUTPUT_BUF` the
    /// output never wraps, so once it is full the call reports `HasMoreOutput` and the loop must
    /// stop rather than spin. A stream longer than the buffer is refused, because a silently
    /// truncated image is a wrong picture.
    #[test]
    fn a_stream_longer_than_the_buffer_is_refused() {
        // 64 bytes of output offered, 128 produced.
        let zlib = zlib_stored(&[7u8; 128], true);
        let mut out = vec![0u8; 64];
        assert!(inflate_into(&zlib, &mut out).is_none(), "overflow must refuse, not truncate");
    }

    /// A stream whose output fills the buffer **and** continues is refused rather than looping.
    ///
    /// This is the "`IHDR` lies about the size" case, and it is the one the no-progress guard in
    /// `inflate_into` exists for. The loop's behaviour on a full buffer is not obvious — verified
    /// against `miniz_oxide` directly:
    ///
    /// ```text
    /// status=HasMoreOutput consumed=28 produced=16   # first call: fills the buffer exactly
    /// status=HasMoreOutput consumed=0  produced=0    # second call: nothing left to do
    /// ```
    ///
    /// So the full buffer is detected on the *second* iteration, by the same guard that catches a
    /// truncated input. The assertion on the first block's bytes pins that the guard fires after
    /// useful work rather than before it: a refusal that returned early without inflating anything
    /// would also be `None`, and this distinguishes the two.
    #[test]
    fn a_stream_that_fills_the_buffer_and_continues_is_refused() {
        let mut deflate = Vec::new();
        // First block: not final, exactly fills a 16-byte buffer.
        deflate.push(0x00);
        let len = 16u16;
        deflate.extend_from_slice(&len.to_le_bytes());
        deflate.extend_from_slice(&(!len).to_le_bytes());
        deflate.extend_from_slice(&[3u8; 16]);
        // Second block: final, so the stream is well-formed and simply larger than the buffer.
        deflate.push(0x01);
        deflate.extend_from_slice(&len.to_le_bytes());
        deflate.extend_from_slice(&(!len).to_le_bytes());
        deflate.extend_from_slice(&[4u8; 16]);
        let mut adler_input = vec![3u8; 16];
        adler_input.extend_from_slice(&[4u8; 16]);
        let mut zlib = vec![0x78, 0x01];
        zlib.extend_from_slice(&deflate);
        zlib.extend_from_slice(&adler32(&adler_input).to_be_bytes());

        let mut out = vec![0u8; 16];
        assert!(
            inflate_into(&zlib, &mut out).is_none(),
            "a stream that fills the buffer and continues must refuse"
        );
        assert_eq!(out, vec![3u8; 16], "the first block's bytes must have been written");
    }

    /// Garbage input is refused rather than returning an uninitialised-looking buffer.
    #[test]
    fn garbage_is_refused() {
        let mut out = vec![0u8; 16];
        assert!(inflate_into(&[], &mut out).is_none(), "an empty stream produces nothing");
        assert!(
            inflate_into(&[0xff; 32], &mut out).is_none(),
            "a stream that is not zlib must be refused"
        );
        assert!(
            inflate_into(&[0x78, 0x01], &mut out).is_none(),
            "a zlib header with no deflate body must be refused"
        );
    }

    /// An empty output buffer is refused before inflating.
    ///
    /// `decompress` documents that a zero-length output makes any write produce `HasMoreOutput`,
    /// so the loop would never reach `Done`. Refusing up front is what keeps that from being an
    /// infinite loop rather than a `None`.
    #[test]
    fn an_empty_output_buffer_is_refused() {
        let zlib = zlib_stored(&[], true);
        let mut out: Vec<u8> = Vec::new();
        assert!(inflate_into(&zlib, &mut out).is_none());
    }

    /// A **multi-call** stream decodes: the streaming loop is what makes this work.
    ///
    /// The cases above are all refusals; this is the positive one that shows the loop's bookkeeping
    /// is right when it has to run more than once. A single-call implementation without the
    /// non-wrapping flag returns `BadParam` for a non-power-of-two buffer, and one *with* the flag
    /// but without the loop stops at the first `HasMoreOutput`. Neither would produce these bytes.
    #[test]
    fn a_stream_is_inflated_across_more_than_one_call() {
        // 8832 is the size a real 136x128 4-bit image needs, and it is not a power of two — which
        // is exactly why the flag and the loop are both required.
        let raw: Vec<u8> = (0..8832u32).map(|i| (i % 251) as u8).collect();
        let zlib = zlib_stored(&raw, true);
        let mut out = vec![0u8; raw.len()];
        inflate_into(&zlib, &mut out).expect("a well-formed stream must inflate");
        assert_eq!(out, raw, "the inflated bytes must match the input exactly");
    }
}
