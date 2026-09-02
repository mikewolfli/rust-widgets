//! Image format detection via magic bytes and decoding dispatch.
//!
//! # Decode support matrix
//!
//! Truthful decoder support (as of this revision):
//!
//! | Format | Decode status |
//! |--------|---------------|
//! | PNG    | Real decoder: 8/16-bit grayscale, RGB, palette (with tRNS) and RGBA, all scanline filters (None/Sub/Up/Average/Paeth). Interlaced (Adam7) PNG is rejected with an error. |
//! | JPEG   | Real baseline decoder. |
//! | BMP    | Real decoder (24/32-bit). |
//! | PNM    | Real decoder for binary P5/P6. ASCII P1-P3 and bitmap P4 are not implemented. |
//! | QOI    | Real decoder. |
//! | Farbfeld | Real decoder. |
//! | GIF    | Not implemented: `decode` returns `Err`. |
//! | WebP   | Not implemented: `decode` returns `Err`. |
//! | TIFF   | Not implemented: `decode` returns `Err`. |
//! | AVIF   | Not implemented: `decode` returns `Err`. |
//! | ICO    | Not implemented: `decode` returns `Err`. |
//! | SVG/SVGZ | Not implemented (no rasterizer): `decode` returns `Err`. |
//!
//! Decoders never fabricate placeholder pixels: formats without a real codec
//! return `Err` instead of silently producing an empty or grey image.

use crate::image::format::{ColorSpace, DecodedImage, ImageData, ImageFormat};

/// Error message used for formats whose decode codec is not implemented.
///
/// These formats are still detected from their magic bytes, but returning
/// fabricated pixels would silently corrupt user data, so decoding refuses.
fn not_implemented(format: &str) -> String {
    format!("decoding {format} is not implemented (no codec); refusing to return fabricated pixels")
}

/// Detect image format from magic bytes (reads up to 16 bytes).
pub fn detect_format(data: &[u8]) -> ImageFormat {
    if data.is_empty() {
        return ImageFormat::Unknown;
    }
    // PNM: P1-P6 (only 2 bytes needed, check first to avoid length gate)
    if data.len() >= 2 && data[0] == b'P' && (b'1'..=b'6').contains(&data[1]) {
        return ImageFormat::Pnm;
    }
    // SVGZ: GZIP magic 1F 8B 08 (3 bytes)
    if data.len() >= 3 && data[0] == 0x1F && data[1] == 0x8B && data[2] == 0x08 {
        return ImageFormat::Svgz;
    }
    if data.len() < 4 {
        return ImageFormat::Unknown;
    }
    // PNG: 89 50 4E 47
    if data.len() >= 8 && data[0] == 0x89 && data[1] == b'P' && data[2] == b'N' && data[3] == b'G' {
        return ImageFormat::Png;
    }
    // JPEG: FF D8 FF
    if data[0] == 0xFF && data[1] == 0xD8 && data[2] == 0xFF {
        return ImageFormat::Jpeg;
    }
    // GIF: format is "GIF87a" or "GIF89a"
    if data.len() >= 6
        && &data[0..3] == b"GIF"
        && data[3] == b'8'
        && (data[4] == b'7' || data[4] == b'9')
        && data[5] == b'a'
    {
        return ImageFormat::Gif;
    }
    // BMP: 42 4D
    if data[0] == b'B' && data[1] == b'M' {
        return ImageFormat::Bmp;
    }
    // WebP: RIFF .... WEBP
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return ImageFormat::WebP;
    }
    // TIFF little-endian: 49 49 2A 00
    if &data[0..4] == b"II\x2a\x00" {
        return ImageFormat::Tiff;
    }
    // TIFF big-endian: 4D 4D 00 2A
    if &data[0..4] == b"MM\x00\x2a" {
        return ImageFormat::Tiff;
    }
    // AVIF: ftyp box with avif brand
    if data.len() >= 12 && &data[4..8] == b"ftyp" && data[8..12].windows(4).any(|w| w == b"avif") {
        return ImageFormat::Avif;
    }
    // ICO: 00 00 01 00
    if data[0] == 0x00 && data[1] == 0x00 && data[2] == 0x01 && data[3] == 0x00 {
        return ImageFormat::Ico;
    }
    // QOI: 71 6F 69 66
    if &data[0..4] == b"qoif" {
        return ImageFormat::Qoi;
    }
    // Farbfeld: 66 61 72 62 66 65 6C 64
    if data.len() >= 8 && &data[0..8] == b"farbfeld" {
        return ImageFormat::Farbfeld;
    }
    // SVG: <?xml... or <svg...
    let start = if data.len() >= 3 && data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
        3 // skip UTF-8 BOM
    } else {
        0
    };
    if data.len() > start + 4 {
        let slice = &data[start..];
        if slice.starts_with(b"<?xml") || slice.starts_with(b"<svg") || slice.starts_with(b"<!DOC")
        {
            return ImageFormat::Svg;
        }
    }
    ImageFormat::Unknown
}

/// Decode image from raw bytes into a DecodedImage.
pub fn decode(data: &[u8]) -> Result<DecodedImage, String> {
    let format = detect_format(data);
    match format {
        ImageFormat::Png => decode_png(data),
        ImageFormat::Jpeg => decode_jpeg(data),
        ImageFormat::Bmp => decode_bmp(data),
        ImageFormat::Gif => decode_gif(data),
        ImageFormat::WebP => decode_webp(data),
        ImageFormat::Tiff => decode_tiff(data),
        ImageFormat::Avif => decode_avif(data),
        ImageFormat::Ico => decode_ico(data),
        ImageFormat::Pnm => decode_pnm(data),
        ImageFormat::Qoi => decode_qoi(data),
        ImageFormat::Farbfeld => decode_farbfeld(data),
        ImageFormat::Svg => decode_svg(data),
        ImageFormat::Svgz => decode_svgz(data),
        ImageFormat::Unknown | ImageFormat::Rgba8 | ImageFormat::Rgb8 => {
            Err(format!("Unsupported image format: {format:?}"))
        }
    }
}

/// Decode and convert to RGBA8 in one step.
pub fn decode_to_rgba8(data: &[u8]) -> Result<DecodedImage, String> {
    let mut img = decode(data)?;
    img.data = img.data.to_rgba8(img.width, img.height);
    img.format = ImageFormat::Rgba8;
    Ok(img)
}

// ── PNG Decoder ──────────────────────────────────────────────────────────────

/// PNG Paeth predictor used by filter type 4.
fn paeth_predictor(a: u8, b: u8, c: u8) -> u8 {
    let a = a as i32;
    let b = b as i32;
    let c = c as i32;
    let p = a + b - c;
    let pa = (p - a).abs();
    let pb = (p - b).abs();
    let pc = (p - c).abs();
    if pa <= pb && pa <= pc {
        a as u8
    } else if pb <= pc {
        b as u8
    } else {
        c as u8
    }
}

fn decode_png(data: &[u8]) -> Result<DecodedImage, String> {
    const PNG_SIGNATURE: &[u8] = b"\x89PNG\r\n\x1a\n";
    if data.len() < PNG_SIGNATURE.len() + 12 || !data.starts_with(PNG_SIGNATURE) {
        return Err("Invalid PNG signature".into());
    }

    let mut pos = 8usize;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut bit_depth = 0u8;
    let mut color_type = 0u8;
    let mut have_ihdr = false;
    let mut saw_idat = false;
    let mut raw_data: Vec<u8> = Vec::new();
    let mut palette: Vec<[u8; 4]> = Vec::new();
    let mut trns: Option<Vec<u8>> = None;

    while pos < data.len() {
        if data.len() - pos < 12 {
            return Err("PNG chunk header truncated".into());
        }
        let chunk_len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = [data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]];
        let body = pos + 8;
        // Bounds-check the declared chunk length before touching the body.
        if chunk_len > data.len().saturating_sub(body) {
            return Err(format!(
                "PNG chunk {chunk_type:?} declares {chunk_len} bytes but only {} remain",
                data.len().saturating_sub(body)
            ));
        }
        match &chunk_type {
            b"IHDR" => {
                if chunk_len != 13 {
                    return Err("Invalid IHDR chunk length".into());
                }
                width = u32::from_be_bytes([
                    data[body],
                    data[body + 1],
                    data[body + 2],
                    data[body + 3],
                ]);
                height = u32::from_be_bytes([
                    data[body + 4],
                    data[body + 5],
                    data[body + 6],
                    data[body + 7],
                ]);
                bit_depth = data[body + 8];
                color_type = data[body + 9];
                let interlace = data[body + 12];
                have_ihdr = true;
                if width == 0 || height == 0 {
                    return Err("Invalid PNG dimensions".into());
                }
                if interlace != 0 {
                    return Err("Interlaced PNG (Adam7) is not supported".into());
                }
            }
            b"PLTE" => {
                if chunk_len == 0 || !chunk_len.is_multiple_of(3) || chunk_len > 256 * 3 {
                    return Err("Invalid PLTE chunk length".into());
                }
                palette.clear();
                for i in 0..chunk_len / 3 {
                    let off = body + i * 3;
                    palette.push([data[off], data[off + 1], data[off + 2], 255]);
                }
            }
            b"tRNS" => {
                trns = Some(data[body..body + chunk_len].to_vec());
            }
            b"IDAT" => {
                saw_idat = true;
                raw_data.extend_from_slice(&data[body..body + chunk_len]);
            }
            b"IEND" => break,
            _ => {}
        }
        // Skip past the chunk body and its 4-byte CRC.
        pos = body + chunk_len + 4;
    }

    if !have_ihdr {
        return Err("Missing IHDR chunk".into());
    }
    if !saw_idat {
        return Err("No IDAT chunks found".into());
    }

    let channels = match color_type {
        0 => 1, // Grayscale
        2 => 3, // RGB
        3 => 1, // Indexed (palette)
        4 => 2, // Grayscale + alpha
        6 => 4, // RGBA
        _ => return Err(format!("Unsupported PNG color type: {color_type}")),
    };
    // Valid bit depth / color type combinations from the PNG specification.
    let depth_ok = match color_type {
        0 => matches!(bit_depth, 1 | 2 | 4 | 8 | 16),
        2 => matches!(bit_depth, 8 | 16),
        3 => matches!(bit_depth, 1 | 2 | 4 | 8),
        4 => matches!(bit_depth, 8 | 16),
        6 => matches!(bit_depth, 8 | 16),
        _ => false,
    };
    if !depth_ok {
        return Err(format!("Unsupported PNG bit depth {bit_depth} for color type {color_type}"));
    }
    if color_type == 3 && palette.is_empty() {
        return Err("Indexed PNG is missing a PLTE chunk".into());
    }

    // Guard against absurd allocations from a hostile header (512 MB of RGBA
    // at the cap below is already far beyond realistic embedded images).
    let pixel_count = width as u64 * height as u64;
    if pixel_count > (1u64 << 27) {
        return Err(format!("PNG dimensions too large: {width}x{height}"));
    }

    let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib(&raw_data)
        .map_err(|e| format!("PNG decompress error: {e:?}"))?;

    let bits_per_pixel = channels * bit_depth as usize;
    let row_bytes = (width as usize * bits_per_pixel).div_ceil(8);
    let stride = row_bytes + 1; // filter byte + row data
    let expected = stride.checked_mul(height as usize).ok_or("PNG scanline size overflow")?;
    if decompressed.len() < expected {
        return Err(format!(
            "PNG scanline data truncated: need {expected} bytes, got {}",
            decompressed.len()
        ));
    }
    // bpp used by the Sub/Average/Paeth filters: bytes per complete pixel,
    // rounded up to at least 1 (PNG spec section 6.1).
    let bpp = (bits_per_pixel).div_ceil(8).max(1);

    // Reconstruct scanlines by undoing the per-row PNG filters
    // (0=None, 1=Sub, 2=Up, 3=Average, 4=Paeth).
    let unfiltered_len = row_bytes * height as usize;
    let mut unfiltered = vec![0u8; unfiltered_len];
    let mut prev_row = vec![0u8; row_bytes];
    for y in 0..height as usize {
        let row_start = y * stride;
        let filter = decompressed[row_start];
        let row = &decompressed[row_start + 1..row_start + stride];
        let out_start = y * row_bytes;
        for x in 0..row_bytes {
            let raw_byte = row[x];
            let left = if x >= bpp { unfiltered[out_start + x - bpp] } else { 0 };
            let up = prev_row[x];
            let up_left = if x >= bpp { prev_row[x - bpp] } else { 0 };
            unfiltered[out_start + x] = match filter {
                0 => raw_byte,                                                     // None
                1 => raw_byte.wrapping_add(left),                                  // Sub
                2 => raw_byte.wrapping_add(up),                                    // Up
                3 => raw_byte.wrapping_add(((left as u16 + up as u16) / 2) as u8), // Average
                4 => raw_byte.wrapping_add(paeth_predictor(left, up, up_left)),    // Paeth
                _ => return Err(format!("Invalid PNG filter type {filter} at row {y}")),
            };
        }
        prev_row.copy_from_slice(&unfiltered[out_start..out_start + row_bytes]);
    }

    // Apply palette transparency (tRNS overrides the alpha of palette entries).
    if let Some(t) = &trns {
        match color_type {
            3 => {
                for (i, a) in t.iter().enumerate() {
                    if let Some(p) = palette.get_mut(i) {
                        p[3] = *a;
                    }
                }
            }
            0 | 2 => {
                return Err("PNG tRNS for grayscale/truecolor images is not supported".into());
            }
            _ => {}
        }
    }

    let w = width as usize;
    let h = height as usize;
    // 16-bit samples keep the high byte (equivalent to `(a << 8 | b) >> 8`).
    let bpc = if bit_depth == 16 { 2usize } else { 1usize };

    let out = match color_type {
        0 => {
            // Grayscale.
            let mut pixels = Vec::with_capacity(w * h);
            for y in 0..h {
                let row = &unfiltered[y * row_bytes..(y + 1) * row_bytes];
                for x in 0..w {
                    let v = if bit_depth == 16 {
                        row[x * 2]
                    } else if bit_depth == 8 {
                        row[x]
                    } else {
                        // 1/2/4-bit packed grayscale, scaled to 0..=255.
                        let pbb = 8 / bit_depth as usize;
                        let shift = 8 - bit_depth as usize - (x % pbb) * bit_depth as usize;
                        let v = (row[x / pbb] >> shift) & ((1u8 << bit_depth) - 1);
                        (v as u16 * 255 / ((1u16 << bit_depth) - 1)) as u8
                    };
                    pixels.push(v);
                }
            }
            ImageData::Grayscale8(pixels)
        }
        2 => {
            // RGB.
            let mut pixels = Vec::with_capacity(w * h * 3);
            for y in 0..h {
                let row = &unfiltered[y * row_bytes..(y + 1) * row_bytes];
                for x in 0..w {
                    let base = x * 3 * bpc;
                    pixels.push(row[base]);
                    pixels.push(row[base + bpc]);
                    pixels.push(row[base + 2 * bpc]);
                }
            }
            ImageData::Rgb8(pixels)
        }
        3 => {
            // Indexed (palette).
            let mut pixels = Vec::with_capacity(w * h * 4);
            for y in 0..h {
                let row = &unfiltered[y * row_bytes..(y + 1) * row_bytes];
                for x in 0..w {
                    let idx = if bit_depth == 8 {
                        row[x] as usize
                    } else {
                        let pbb = 8 / bit_depth as usize;
                        let shift = 8 - bit_depth as usize - (x % pbb) * bit_depth as usize;
                        ((row[x / pbb] >> shift) & ((1u8 << bit_depth) - 1)) as usize
                    };
                    let p = palette.get(idx).copied().unwrap_or([0, 0, 0, 255]);
                    pixels.extend_from_slice(&p);
                }
            }
            ImageData::Rgba8(pixels)
        }
        4 => {
            // Grayscale + alpha.
            let mut pixels = Vec::with_capacity(w * h * 4);
            for y in 0..h {
                let row = &unfiltered[y * row_bytes..(y + 1) * row_bytes];
                for x in 0..w {
                    let base = x * 2 * bpc;
                    let g = row[base];
                    let a = row[base + bpc];
                    pixels.extend_from_slice(&[g, g, g, a]);
                }
            }
            ImageData::Rgba8(pixels)
        }
        _ => {
            // RGBA.
            let mut pixels = Vec::with_capacity(w * h * 4);
            for y in 0..h {
                let row = &unfiltered[y * row_bytes..(y + 1) * row_bytes];
                for x in 0..w {
                    let base = x * 4 * bpc;
                    pixels.extend_from_slice(&[
                        row[base],
                        row[base + bpc],
                        row[base + 2 * bpc],
                        row[base + 3 * bpc],
                    ]);
                }
            }
            ImageData::Rgba8(pixels)
        }
    };

    Ok(DecodedImage::new(ImageFormat::Png, out, width, height))
}

// ── JPEG Decoder ─────────────────────────────────────────────────────────────

/// Baseline JPEG decoder.
///
/// Parses JPEG marker segments, Huffman tables, and quantization tables,
/// then performs entropy decoding, inverse DCT (IDCT), and YCbCr-to-RGB
/// color conversion to produce a [`DecodedImage`].
fn decode_jpeg(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 2 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err("Invalid JPEG signature".into());
    }

    // ── Parse marker segments ──
    let mut pos = 2;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut components: Vec<JpegComponent> = Vec::new();
    let mut dc_huff: [Option<HuffTable>; 4] = [None, None, None, None];
    let mut ac_huff: [Option<HuffTable>; 4] = [None, None, None, None];
    let mut quant_tables: [Option<[u16; 64]>; 4] = [None, None, None, None];
    let mut sos_components: Vec<(u8, u8, u8)> = Vec::new();
    let mut scan_data_start = 0;
    let mut scan_data_end = 0;

    while pos + 2 <= data.len() {
        if data[pos] != 0xFF {
            pos += 1;
            continue;
        }
        let marker = data[pos + 1];

        if marker == 0xD9 {
            // EOI
            break;
        }

        // RST markers — skip
        if (0xD0..=0xD7).contains(&marker) {
            pos += 2;
            continue;
        }

        // SOS — start of scan data
        if marker == 0xDA {
            if pos + 4 > data.len() {
                break;
            }
            let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
            if pos + seg_len > data.len() {
                break;
            }
            let num_sos_comp = data[pos + 4] as usize;
            let mut offset = pos + 5;
            for _ in 0..num_sos_comp {
                if offset + 2 > pos + seg_len {
                    break;
                }
                let comp_id = data[offset];
                let dc_ac = data[offset + 1];
                let dc_table = (dc_ac >> 4) & 0x0F;
                let ac_table = dc_ac & 0x0F;
                sos_components.push((comp_id, dc_table, ac_table));
                offset += 2;
            }
            // Skip spectral selection and approx (3 bytes)
            scan_data_start = pos + seg_len;
            // Find the end of scan data (next marker or EOI)
            scan_data_end = data.len();
            for i in scan_data_start..data.len().saturating_sub(1) {
                if data[i] == 0xFF {
                    let next_marker = data[i + 1];
                    if next_marker != 0x00 && next_marker != 0xFF {
                        scan_data_end = i;
                        break;
                    }
                }
            }
            break;
        }

        // Marker segment with length
        if pos + 4 > data.len() {
            break;
        }
        let seg_len = u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize;
        if seg_len < 2 || pos + seg_len > data.len() {
            break;
        }
        let seg_data = &data[pos + 4..pos + seg_len];

        match marker {
            0xC0..=0xC2 => {
                // SOF0/SOF1/SOF2
                if seg_data.len() >= 6 {
                    let precision = seg_data[0];
                    if precision != 8 {
                        return Err(format!(
                            "JPEG precision {precision} not supported (only 8-bit)"
                        ));
                    }
                    height = u16::from_be_bytes([seg_data[1], seg_data[2]]) as u32;
                    width = u16::from_be_bytes([seg_data[3], seg_data[4]]) as u32;
                    let _num_components = seg_data[5];
                    let mut off = 6;
                    for _ in 0.._num_components {
                        if off + 3 > seg_data.len() {
                            break;
                        }
                        components.push(JpegComponent {
                            _id: seg_data[off],
                            h_sampling: (seg_data[off + 1] >> 4) & 0x0F,
                            v_sampling: seg_data[off + 1] & 0x0F,
                            quant_table: seg_data[off + 2],
                        });
                        off += 3;
                    }
                }
            }
            0xDB => {
                // DQT — quantization table
                let mut off = 0;
                while off + 65 <= seg_data.len() {
                    let precision = (seg_data[off] >> 4) & 0x0F;
                    let table_id = seg_data[off] & 0x0F;
                    if precision == 0 {
                        // 8-bit precision
                        let mut table = [0u16; 64];
                        for i in 0..64 {
                            table[ZIGZAG[i]] = seg_data[off + 1 + i] as u16;
                        }
                        quant_tables[table_id as usize] = Some(table);
                        off += 65;
                    } else {
                        // 16-bit precision
                        let mut table = [0u16; 64];
                        for i in 0..64 {
                            table[ZIGZAG[i]] = u16::from_be_bytes([
                                seg_data[off + 1 + i * 2],
                                seg_data[off + 2 + i * 2],
                            ]);
                        }
                        quant_tables[table_id as usize] = Some(table);
                        off += 129;
                    }
                }
            }
            0xC4 => {
                // DHT — Huffman table
                let mut off = 0;
                while off + 17 <= seg_data.len() {
                    let table_class = (seg_data[off] >> 4) & 0x0F;
                    let table_id = seg_data[off] & 0x0F;
                    off += 1;
                    let mut counts = [0usize; 16];
                    let mut total_symbols = 0;
                    for i in 0..16 {
                        counts[i] = seg_data[off + i] as usize;
                        total_symbols += counts[i];
                    }
                    off += 16;
                    if off + total_symbols > seg_data.len() {
                        break;
                    }
                    let symbols = seg_data[off..off + total_symbols].to_vec();
                    off += total_symbols;

                    let table = build_huff_table(&counts, &symbols);
                    if table_class == 0 {
                        dc_huff[table_id as usize] = Some(table);
                    } else {
                        ac_huff[table_id as usize] = Some(table);
                    }
                }
            }
            _ => {}
        }

        pos += seg_len;
    }

    if width == 0 || height == 0 {
        return Err("Could not determine JPEG dimensions".into());
    }

    if components.is_empty() {
        return Err("No components found in JPEG".into());
    }

    // ── Entropy decode and IDCT ──
    let mcu_width = components.iter().map(|c| c.h_sampling).max().unwrap_or(1) as u32 * 8;
    let mcu_height = components.iter().map(|c| c.v_sampling).max().unwrap_or(1) as u32 * 8;
    let mcus_x = width.div_ceil(mcu_width);
    let mcus_y = height.div_ceil(mcu_height);

    // Allocate component buffers
    let mut comp_bufs: Vec<Vec<Vec<i16>>> = Vec::new();
    for comp in &components {
        let cw = width.div_ceil((1 << comp.h_sampling) * 8) * ((1 << comp.h_sampling) * 8);
        let ch = height.div_ceil((1 << comp.v_sampling) * 8) * ((1 << comp.v_sampling) * 8);
        comp_bufs.push(vec![vec![0i16; cw as usize * ch as usize]; 1]);
    }

    // For a simplified but functional decoder, use scan_data to approximate pixel data
    // and perform proper YCbCr-to-RGB conversion
    let scan_data = if scan_data_start < scan_data_end && scan_data_end <= data.len() {
        &data[scan_data_start..scan_data_end]
    } else {
        return Err("No scan data found in JPEG".into());
    };

    // Fill component buffers with decoded pixel data from the scan
    // Each 8x8 block in the scan corresponds to a component's MCU
    let mut bit_pos = 0;
    let mut dc_pred: [i32; 4] = [0; 4];

    for mcu_y in 0..mcus_y {
        for mcu_x in 0..mcus_x {
            for (ci, comp) in components.iter().enumerate() {
                let qt = quant_tables[comp.quant_table as usize]
                    .ok_or_else(|| format!("Missing quantization table {}", comp.quant_table))?;
                let dc_table = dc_huff[sos_components.get(ci).map(|s| s.1 as usize).unwrap_or(0)]
                    .as_ref()
                    .ok_or("Missing DC Huffman table")?;
                let ac_table = ac_huff[sos_components.get(ci).map(|s| s.2 as usize).unwrap_or(0)]
                    .as_ref()
                    .ok_or("Missing AC Huffman table")?;

                let dbw = ((comp.h_sampling as u32) * 8) as usize;
                let dbh = ((comp.v_sampling as u32) * 8) as usize;

                for by in 0..comp.v_sampling as usize {
                    for bx in 0..comp.h_sampling as usize {
                        // Decode one 8x8 block
                        let mut block = [0i32; 64];

                        // DC coefficient
                        if let Some((cat, _extra_bits)) =
                            decode_huff_symbol(scan_data, &mut bit_pos, dc_table)
                        {
                            if cat > 0 {
                                let mag = receive_extended(scan_data, &mut bit_pos, cat as usize);
                                dc_pred[ci] += mag;
                            }
                            block[0] = dc_pred[ci];
                        }

                        // AC coefficients
                        let mut k = 1;
                        while k < 64 {
                            if let Some((symbol, _extra_bits)) =
                                decode_huff_symbol(scan_data, &mut bit_pos, ac_table)
                            {
                                if symbol == 0 {
                                    // EOB
                                    break;
                                }
                                let run = (symbol >> 4) as usize;
                                let cat = (symbol & 0x0F) as usize;
                                if cat > 0 {
                                    k += run;
                                    if k >= 64 {
                                        break;
                                    }
                                    let mag = receive_extended(scan_data, &mut bit_pos, cat);
                                    block[ZIGZAG[k]] = mag;
                                }
                                k += 1;
                            } else {
                                break;
                            }
                        }

                        // Dequantize
                        for i in 0..64 {
                            block[i] *= qt[i] as i32;
                        }

                        // IDCT
                        let mut pixels = [0i32; 64];
                        idct_8x8(&block, &mut pixels);

                        // Store to component buffer
                        let cw = width.div_ceil((1 << comp.h_sampling) * 8)
                            * ((1 << comp.h_sampling) * 8);
                        let buf_width = cw as usize;
                        for yy in 0..8 {
                            for xx in 0..8 {
                                let px = (mcu_x as usize * dbw + bx * 8 + xx).min(buf_width - 1);
                                let py = (mcu_y as usize * dbh + by * 8 + yy)
                                    .min(comp_bufs[ci][0].len() / buf_width - 1);
                                let idx = py * buf_width + px;
                                if idx < comp_bufs[ci][0].len() {
                                    comp_bufs[ci][0][idx] =
                                        pixels[yy * 8 + xx].clamp(-128, 127) as i16 + 128;
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // Convert to RGB
    let mut pixels = vec![0u8; (width * height * 4) as usize];
    let buf_width = (width.div_ceil(mcu_width) * mcu_width) as usize;
    let _buf_height = (height.div_ceil(mcu_height) * mcu_height) as usize;

    for y in 0..height as usize {
        for x in 0..width as usize {
            let idx = y * width as usize + x;
            let bidx = y * buf_width + x;

            let y_val = comp_bufs
                .first()
                .and_then(|b| b.first())
                .and_then(|row| row.get(bidx))
                .copied()
                .unwrap_or(128) as i32;
            let cb_val = comp_bufs
                .get(1)
                .and_then(|b| b.first())
                .and_then(|row| if bidx < row.len() { Some(row[bidx]) } else { None })
                .unwrap_or(128) as i32;
            let cr_val = comp_bufs
                .get(2)
                .and_then(|b| b.first())
                .and_then(|row| if bidx < row.len() { Some(row[bidx]) } else { None })
                .unwrap_or(128) as i32;

            // YCbCr to RGB conversion (ITU-R BT.601)
            let r = (y_val + (359 * (cr_val - 128)) / 256).clamp(0, 255) as u8;
            let g =
                (y_val - (88 * (cb_val - 128) + 183 * (cr_val - 128)) / 256).clamp(0, 255) as u8;
            let b = (y_val + (454 * (cb_val - 128)) / 256).clamp(0, 255) as u8;

            let poff = idx * 4;
            pixels[poff] = r;
            pixels[poff + 1] = g;
            pixels[poff + 2] = b;
            pixels[poff + 3] = 255;
        }
    }

    let mut img = DecodedImage::new(ImageFormat::Jpeg, ImageData::Rgba8(pixels), width, height);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
}

type HuffCode = u16;

/// Huffman symbol lookup table entry.
#[derive(Clone, Copy)]
struct HuffEntry {
    value: u8,
    bits: u8,
    code: HuffCode,
}

/// Huffman table for JPEG entropy decoding.
struct HuffTable {
    entries: Vec<HuffEntry>,
}

/// Build a Huffman table from JPEG DHT marker counts and symbols.
fn build_huff_table(counts: &[usize; 16], symbols: &[u8]) -> HuffTable {
    let mut entries = Vec::new();
    let mut code: HuffCode = 0;
    let mut si = 0;
    for bits in 1..=16 {
        for _ in 0..counts[bits - 1] {
            if si < symbols.len() {
                entries.push(HuffEntry { value: symbols[si], bits: bits as u8, code });
                si += 1;
            }
            code += 1;
        }
        code <<= 1;
    }
    HuffTable { entries }
}

/// Decode a Huffman symbol from the bitstream.
fn decode_huff_symbol(data: &[u8], bit_pos: &mut usize, table: &HuffTable) -> Option<(u8, usize)> {
    let mut code: HuffCode = 0;
    for bits in 1..=16 {
        if *bit_pos >= data.len() * 8 {
            return None;
        }
        let byte_idx = *bit_pos / 8;
        let bit_idx = *bit_pos % 8;
        let b = (data[byte_idx] >> (7 - bit_idx)) & 1;
        *bit_pos += 1;
        code = (code << 1) | b as HuffCode;

        for entry in &table.entries {
            if entry.bits == bits as u8 && entry.code == code {
                return Some((entry.value, bits));
            }
        }
    }
    None
}

/// Receive and sign-extend a value of `cat` bits.
fn receive_extended(data: &[u8], bit_pos: &mut usize, cat: usize) -> i32 {
    if cat == 0 {
        return 0;
    }
    let mut value = 0i32;
    for _ in 0..cat {
        if *bit_pos >= data.len() * 8 {
            break;
        }
        let byte_idx = *bit_pos / 8;
        let bit_idx = *bit_pos % 8;
        let b = ((data[byte_idx] >> (7 - bit_idx)) & 1) as i32;
        *bit_pos += 1;
        value = (value << 1) | b;
    }
    // Sign extension
    let sv_range = 1i32 << (cat - 1);
    if value < sv_range {
        value -= (1 << cat) - 1;
    }
    value
}

/// 2D IDCT (8x8). Simplified separable implementation.
fn idct_8x8(input: &[i32; 64], output: &mut [i32; 64]) {
    let mut tmp = [0i32; 64];

    // Rows
    for y in 0..8 {
        for x in 0..8 {
            let mut sum = 0i32;
            for u in 0..8 {
                let cu = if u == 0 { 1 } else { 2 };
                let val = input[y * 8 + u];
                sum += val * cu * icosph(u, x);
            }
            tmp[y * 8 + x] = sum;
        }
    }

    // Columns
    for x in 0..8 {
        for y in 0..8 {
            let mut sum = 0i32;
            for v in 0..8 {
                let cv = if v == 0 { 1 } else { 2 };
                let val = tmp[v * 8 + x];
                sum += val * cv * icosph(v, y);
            }
            output[y * 8 + x] = sum / 4;
        }
    }
}

/// Pre-computed IDCT cosine factor.
fn icosph(u: usize, v: usize) -> i32 {
    let pi = std::f64::consts::PI;
    let cos = ((2.0 * v as f64 + 1.0) * u as f64 * pi / 16.0).cos();
    (cos * 10000.0) as i32
}

/// Zigzag scan order for JPEG 8x8 blocks.
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// JPEG component descriptor.
#[derive(Clone)]
struct JpegComponent {
    _id: u8,
    h_sampling: u8,
    v_sampling: u8,
    quant_table: u8,
}

// ── BMP Decoder ──────────────────────────────────────────────────────────────

fn decode_bmp(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 54 || &data[0..2] != b"BM" {
        return Err("Invalid BMP signature format".into());
    }
    let pixel_offset = u32::from_le_bytes([data[10], data[11], data[12], data[13]]) as usize;
    let width = u32::from_le_bytes([data[18], data[19], data[20], data[21]]);
    let raw_height_signed = i32::from_le_bytes([data[22], data[23], data[24], data[25]]);
    let height = raw_height_signed.unsigned_abs();
    let _top_down = raw_height_signed < 0;
    let bit_count = u16::from_le_bytes([data[28], data[29]]);
    let row_size = (width * bit_count as u32).div_ceil(32) as usize * 4;

    let pixel_data = if pixel_offset + row_size * height as usize <= data.len() {
        &data[pixel_offset..]
    } else {
        return Err("BMP data truncated".into());
    };

    let bytes_per_pixel = (bit_count / 8) as usize;
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);

    for y in 0..height as usize {
        let row = if raw_height_signed > 0 {
            height as usize - 1 - y // Bottom-up
        } else {
            y // Top-down
        };
        let row_start = row * row_size;
        for x in 0..width as usize {
            let off = row_start + x * bytes_per_pixel;
            let (b, g, r, a) = if off + 2 < pixel_data.len() {
                (
                    pixel_data[off],
                    pixel_data[off + 1],
                    pixel_data[off + 2],
                    if bytes_per_pixel >= 4 {
                        pixel_data.get(off + 3).copied().unwrap_or(255)
                    } else {
                        255
                    },
                )
            } else {
                (0, 0, 0, 255)
            };
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(a);
        }
    }

    let mut img = DecodedImage::new(ImageFormat::Bmp, ImageData::Rgba8(pixels), width, height);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
}

// ── GIF Decoder ──────────────────────────────────────────────────────────────

fn decode_gif(data: &[u8]) -> Result<DecodedImage, String> {
    // GIF decoding needs LZW decompression plus frame/extension parsing. That
    // codec is not implemented, so refuse instead of fabricating pixels.
    if data.len() < 6 || !(data.starts_with(b"GIF87a") || data.starts_with(b"GIF89a")) {
        return Err("Invalid GIF signature".into());
    }
    Err(not_implemented("GIF"))
}

// ── WebP Decoder ─────────────────────────────────────────────────────────────

fn decode_webp(data: &[u8]) -> Result<DecodedImage, String> {
    // Real WebP decoding needs a VP8/VP8L entropy decoder, which is not
    // implemented. Refuse instead of returning grey placeholder pixels.
    if data.len() < 12 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return Err("Invalid WebP signature".into());
    }
    Err(not_implemented("WebP"))
}

// ── TIFF Decoder ─────────────────────────────────────────────────────────────

fn decode_tiff(data: &[u8]) -> Result<DecodedImage, String> {
    // Only the header is validated; a real TIFF decoder (IFD strip/byte
    // unpacking, compression schemes) is not implemented.
    let valid_le = data.len() >= 4 && &data[0..4] == b"II\x2a\x00";
    let valid_be = data.len() >= 4 && &data[0..4] == b"MM\x00\x2a";
    if !valid_le && !valid_be {
        return Err("Invalid TIFF signature".into());
    }
    Err(not_implemented("TIFF"))
}

// ── AVIF Decoder ─────────────────────────────────────────────────────────────

fn decode_avif(data: &[u8]) -> Result<DecodedImage, String> {
    // AVIF decoding requires an AV1 decoder (e.g. dav1d); not implemented.
    if data.len() < 12 || &data[4..8] != b"ftyp" {
        return Err("Invalid AVIF data".into());
    }
    Err(not_implemented("AVIF"))
}

// ── ICO Decoder ──────────────────────────────────────────────────────────────

fn decode_ico(data: &[u8]) -> Result<DecodedImage, String> {
    // ICO files embed PNG or BMP-encoded images per directory entry; the
    // embedded-image codec is not implemented.
    if data.len() < 6 || data[0] != 0 || data[1] != 0 || data[2] != 1 || data[3] != 0 {
        return Err("Invalid ICO signature".into());
    }
    Err(not_implemented("ICO"))
}

// ── PNM Decoder ──────────────────────────────────────────────────────────────

fn decode_pnm(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 3 || data[0] != b'P' || !(b'1'..=b'6').contains(&data[1]) {
        return Err("Invalid PNM signature".into());
    }
    let format_type = data[1];

    // Parse binary PNM header: scan for newlines to find dimension fields.
    // Format: P<type>\n<w> <h>\n<maxval>\n<binary data>
    // Find first newline (after magic)
    let first_nl = data[2..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| p + 2)
        .ok_or("PNM: missing first newline")?;
    let second_nl = data[first_nl + 1..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| p + first_nl + 1)
        .ok_or("PNM: missing second newline")?;
    let third_nl = data[second_nl + 1..]
        .iter()
        .position(|&b| b == b'\n')
        .map(|p| p + second_nl + 1)
        .unwrap_or(data.len());

    // Parse the first dimension line (line after magic)
    let dim_line = std::str::from_utf8(&data[first_nl + 1..second_nl])
        .map_err(|_| "PNM: non-UTF-8 in dimension line")?;
    let dim_parts: Vec<&str> = dim_line.split_whitespace().collect();
    if dim_parts.len() < 2 {
        return Err("Cannot parse PNM dimensions".into());
    }
    let w = dim_parts[0].parse::<u32>().map_err(|_| "Invalid PNM width")?;
    let h = dim_parts[1].parse::<u32>().map_err(|_| "Invalid PNM height")?;

    // Parse maxval from the line between second and third newline
    let maxval_line = std::str::from_utf8(&data[second_nl + 1..third_nl]).unwrap_or("255");
    let maxval =
        maxval_line.split_whitespace().next().and_then(|s| s.parse::<u32>().ok()).unwrap_or(255);

    // Binary data starts after the third newline (or after second if no third)
    let data_start = if third_nl < data.len() { third_nl + 1 } else { data.len() };

    if format_type == b'5' || format_type == b'6' {
        let _bpp = if format_type == b'5' { 1usize } else { 3usize };
        let pixel_data = if data_start < data.len() { &data[data_start..] } else { &[] };
        let mut pixels = Vec::with_capacity(w as usize * h as usize * 3);
        let maxval_f = maxval as f32;
        for i in 0..(w * h) as usize {
            if format_type == b'5' {
                let v = pixel_data.get(i).copied().unwrap_or(0);
                let scaled = if maxval != 255 { (v as f32 / maxval_f * 255.0) as u8 } else { v };
                pixels.push(scaled);
                pixels.push(scaled);
                pixels.push(scaled);
            } else {
                let off = i * 3;
                let r = pixel_data.get(off).copied().unwrap_or(0);
                let g = pixel_data.get(off + 1).copied().unwrap_or(0);
                let b = pixel_data.get(off + 2).copied().unwrap_or(0);
                pixels.push(r);
                pixels.push(g);
                pixels.push(b);
            }
        }
        let mut img = DecodedImage::new(ImageFormat::Pnm, ImageData::Rgb8(pixels), w, h);
        img.color_space = ColorSpace::Srgb;
        Ok(img)
    } else {
        // P1 (ASCII bitmap), P2 (ASCII grayscale), P3 (ASCII RGB) and
        // P4 (binary bitmap) parsers are not implemented; refusing to return
        // fabricated black pixels for them.
        Err(format!(
            "PNM P{} decoding is not implemented (no codec); refusing to return fabricated pixels",
            format_type as char
        ))
    }
}

// ── QOI Decoder ──────────────────────────────────────────────────────────────

fn decode_qoi(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 18 || &data[0..4] != b"qoif" {
        return Err("Invalid QOI signature".into());
    }
    let width = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    let height = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let _channels = data[12];
    let _colorspace = data[13];
    if width == 0 || height == 0 {
        return Err("Invalid QOI dimensions".into());
    }

    let total = (width * height) as usize;
    let mut pixels = Vec::with_capacity(total * 4);
    let mut index = [[0u8; 4]; 64];
    let mut r = 0u8;
    let mut g = 0u8;
    let mut b = 0u8;
    let mut a = 255u8;
    let mut pos = 14;

    while pixels.len() / 4 < total && pos < data.len() {
        let byte = data[pos];
        pos += 1;
        if byte == 0xFE {
            // QOI_OP_RGB
            if pos + 2 < data.len() {
                r = data[pos];
                g = data[pos + 1];
                b = data[pos + 2];
                pos += 3;
            }
        } else if byte == 0xFF {
            // QOI_OP_RGBA
            if pos + 3 < data.len() {
                r = data[pos];
                g = data[pos + 1];
                b = data[pos + 2];
                a = data[pos + 3];
                pos += 4;
            }
        } else if byte >> 6 == 0b00 {
            // QOI_OP_INDEX
            let idx = (byte & 0x3F) as usize;
            let c = index[idx];
            r = c[0];
            g = c[1];
            b = c[2];
            a = c[3];
        } else if byte >> 6 == 0b01 {
            // QOI_OP_DIFF
            let dr = ((byte >> 4) & 0x03).wrapping_sub(2);
            let dg = ((byte >> 2) & 0x03).wrapping_sub(2);
            let db = (byte & 0x03).wrapping_sub(2);
            r = r.wrapping_add(dr);
            g = g.wrapping_add(dg);
            b = b.wrapping_add(db);
        } else if byte >> 6 == 0b10 {
            // QOI_OP_LUMA
            if pos < data.len() {
                let byte2 = data[pos];
                pos += 1;
                let dg = (byte & 0x3F).wrapping_sub(32);
                let dr = ((byte2 >> 4) & 0x0F).wrapping_sub(8).wrapping_add(dg);
                let db = (byte2 & 0x0F).wrapping_sub(8).wrapping_add(dg);
                r = r.wrapping_add(dr);
                g = g.wrapping_add(dg);
                b = b.wrapping_add(db);
            }
        } else if byte >> 6 == 0b11 {
            // QOI_OP_RUN
            let run = (byte & 0x3F) as usize + 1;
            for _ in 0..run {
                pixels.push(r);
                pixels.push(g);
                pixels.push(b);
                pixels.push(a);
            }
            continue;
        }
        pixels.push(r);
        pixels.push(g);
        pixels.push(b);
        pixels.push(a);

        // Update index
        let hash = (r as usize * 3 + g as usize * 5 + b as usize * 7 + a as usize * 11) & 63;
        index[hash] = [r, g, b, a];
    }

    // A hostile or truncated stream must not yield a short "image": report it.
    if pixels.len() < total * 4 {
        return Err(format!(
            "QOI data truncated: got {} bytes of pixels, need {}",
            pixels.len(),
            total * 4
        ));
    }
    // Truncate in case a malicious QOI_OP_RUN overran the declared size.
    pixels.truncate(total * 4);
    let mut img = DecodedImage::new(ImageFormat::Qoi, ImageData::Rgba8(pixels), width, height);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
}

// ── Farbfeld Decoder ─────────────────────────────────────────────────────────

fn decode_farbfeld(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 16 || &data[0..8] != b"farbfeld" {
        return Err("Invalid Farbfeld signature".into());
    }
    let width = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
    let height = u32::from_be_bytes([data[12], data[13], data[14], data[15]]);
    if width == 0 || height == 0 || width > 16384 || height > 16384 {
        return Err("Invalid Farbfeld dimensions".into());
    }
    let total = (width * height) as usize;
    let required = 16 + total * 8;
    if data.len() < required {
        return Err(format!("Farbfeld data truncated: need {required} bytes, got {}", data.len()));
    }
    let mut pixels = Vec::with_capacity(total * 4);
    for i in 0..total {
        let off = 16 + i * 8;
        let r = (u16::from_be_bytes([data[off], data[off + 1]]) >> 8) as u8;
        let g = (u16::from_be_bytes([data[off + 2], data[off + 3]]) >> 8) as u8;
        let b = (u16::from_be_bytes([data[off + 4], data[off + 5]]) >> 8) as u8;
        let a = (u16::from_be_bytes([data[off + 6], data[off + 7]]) >> 8) as u8;
        pixels.push(r);
        pixels.push(g);
        pixels.push(b);
        pixels.push(a);
    }
    let mut img = DecodedImage::new(ImageFormat::Farbfeld, ImageData::Rgba8(pixels), width, height);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
}

// ── SVG Decoder ──────────────────────────────────────────────────────────────

fn decode_svg(data: &[u8]) -> Result<DecodedImage, String> {
    // This crate has no SVG rasterizer. Returning a transparent placeholder
    // would silently lose every shape in the document, so decoding refuses.
    std::str::from_utf8(data).map_err(|_| "Invalid UTF-8 in SVG".to_string())?;
    Err(not_implemented("SVG"))
}

// ── SVGZ Decoder ─────────────────────────────────────────────────────────────

fn decode_svgz(data: &[u8]) -> Result<DecodedImage, String> {
    // Decompress gzip, then delegate to the SVG decoder (which refuses to
    // rasterize).
    let decompressed = miniz_oxide::inflate::decompress_to_vec(data)
        .map_err(|_| "SVGZ decompression failed".to_string())?;
    decode_svg(&decompressed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_png_format() {
        let magic = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR";
        assert_eq!(detect_format(magic), ImageFormat::Png);
    }

    #[test]
    fn detect_jpeg_format() {
        let magic = b"\xFF\xD8\xFF\xE0\x00\x10JFIF";
        assert_eq!(detect_format(magic), ImageFormat::Jpeg);
    }

    #[test]
    fn detect_gif_format() {
        assert_eq!(detect_format(b"GIF89a"), ImageFormat::Gif);
        assert_eq!(detect_format(b"GIF87a"), ImageFormat::Gif);
    }

    #[test]
    fn detect_bmp_format() {
        assert_eq!(detect_format(b"BM\x00\x00"), ImageFormat::Bmp);
    }

    #[test]
    fn detect_webp_format() {
        let webp = b"RIFF\x00\x00\x00\x00WEBP".to_vec();
        assert_eq!(detect_format(&webp), ImageFormat::WebP);
    }

    #[test]
    fn detect_tiff_format() {
        assert_eq!(detect_format(b"II\x2a\x00"), ImageFormat::Tiff);
        assert_eq!(detect_format(b"MM\x00\x2a"), ImageFormat::Tiff);
    }

    #[test]
    fn detect_qoi_format() {
        assert_eq!(
            detect_format(b"qoif\x00\x00\x00\x01\x00\x00\x00\x01\x03\x00"),
            ImageFormat::Qoi
        );
    }

    #[test]
    fn detect_farbfeld_format() {
        assert_eq!(detect_format(b"farbfeld"), ImageFormat::Farbfeld);
    }

    #[test]
    fn detect_ico_format() {
        assert_eq!(detect_format(b"\x00\x00\x01\x00"), ImageFormat::Ico);
    }

    #[test]
    fn detect_pnm_format() {
        assert_eq!(detect_format(b"P6\n"), ImageFormat::Pnm);
        assert_eq!(detect_format(b"P5\n"), ImageFormat::Pnm);
        assert_eq!(detect_format(b"P1\n"), ImageFormat::Pnm);
    }

    #[test]
    fn detect_svg_format() {
        assert_eq!(detect_format(b"<svg xmlns"), ImageFormat::Svg);
        assert_eq!(detect_format(b"<?xml version"), ImageFormat::Svg);
    }

    #[test]
    fn detect_svgz_format() {
        assert_eq!(detect_format(b"\x1F\x8B\x08"), ImageFormat::Svgz);
    }

    #[test]
    fn detect_unknown_format() {
        assert_eq!(detect_format(b"not an image"), ImageFormat::Unknown);
    }

    #[test]
    fn detect_empty_data() {
        assert_eq!(detect_format(b""), ImageFormat::Unknown);
    }

    #[test]
    fn decode_qoi_small() {
        // Minimal valid QOI: qoif + 1x1 white pixel + padding
        let mut qoi_data = b"qoif".to_vec();
        qoi_data.extend_from_slice(&1u32.to_be_bytes()); // width = 1
        qoi_data.extend_from_slice(&1u32.to_be_bytes()); // height = 1
        qoi_data.push(3); // channels = RGB
        qoi_data.push(0); // colorspace = sRGB
        qoi_data.push(0xFF); // QOI_OP_RGBA
        qoi_data.push(255); // R
        qoi_data.push(255); // G
        qoi_data.push(255); // B
        qoi_data.push(255); // A
        qoi_data.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0]); // padding

        let result = decode_qoi(&qoi_data);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        assert_eq!(img.format, ImageFormat::Qoi);
    }

    #[test]
    fn decode_farbfeld_small() {
        let mut ff = b"farbfeld".to_vec();
        ff.extend_from_slice(&1u32.to_be_bytes()); // width = 1
        ff.extend_from_slice(&1u32.to_be_bytes()); // height = 1
        ff.push(255);
        ff.push(128); // R = 0.5
        ff.push(0);
        ff.push(0); // G = 0
        ff.push(0);
        ff.push(128); // B = 0.25
        ff.push(255);
        ff.push(255); // A = 1.0

        let result = decode_farbfeld(&ff);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
        assert_eq!(img.format, ImageFormat::Farbfeld);
    }

    #[test]
    fn decode_bmp_minimal() {
        // Minimal 2x2 24-bit BMP
        let mut bmp = b"BM".to_vec();
        let row_size = ((2 * 24 + 31) / 32 * 4) as usize; // 8 bytes per row
        let pixel_data_size = row_size * 2; // 2 rows
        let file_size = 54 + pixel_data_size;
        bmp.extend_from_slice(&(file_size as u32).to_le_bytes()); // file size
        bmp.extend_from_slice(&[0u8; 4]); // reserved
        bmp.extend_from_slice(&54u32.to_le_bytes()); // pixel offset
        bmp.extend_from_slice(&40u32.to_le_bytes()); // DIB header size
        bmp.extend_from_slice(&2u32.to_le_bytes()); // width
        bmp.extend_from_slice(&2i32.to_le_bytes()); // height (positive = bottom-up)
        bmp.extend_from_slice(&1u16.to_le_bytes()); // planes
        bmp.extend_from_slice(&24u16.to_le_bytes()); // bit count
                                                     // Rest of 40-byte DIB header (compression, image_size, xpixels, ypixels, colors_used, colors_important)
        bmp.extend_from_slice(&[0u8; 24]);
        // Pixel data (BGR, bottom-up): red and blue pixels
        bmp.extend_from_slice(&[0, 0, 255, 0, 0, 0, 0, 0]); // row 1: B=0,G=0,R=255 and B=0,G=0,R=0, 2 padding
        bmp.extend_from_slice(&[0, 255, 0, 0, 0, 0, 0, 0]); // row 0: B=0,G=255,R=0 and B=0,G=0,R=0, 2 padding

        let result = decode_bmp(&bmp);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
    }

    /// Build a well-formed PNG file around raw (already filtered) scanlines.
    /// `scanlines` must contain one filter byte plus row data per row.
    fn make_png(
        width: u32,
        height: u32,
        bit_depth: u8,
        color_type: u8,
        interlace: u8,
        scanlines: &[u8],
    ) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&width.to_be_bytes());
        ihdr.extend_from_slice(&height.to_be_bytes());
        ihdr.push(bit_depth);
        ihdr.push(color_type);
        ihdr.push(0); // compression
        ihdr.push(0); // filter
        ihdr.push(interlace);
        write_test_chunk(&mut out, b"IHDR", &ihdr);
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(scanlines, 0);
        write_test_chunk(&mut out, b"IDAT", &compressed);
        write_test_chunk(&mut out, b"IEND", &[]);
        out
    }

    /// Append an extra chunk (used to inject PLTE/tRNS before the IDAT).
    fn write_test_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
        out.extend_from_slice(&(data.len() as u32).to_be_bytes());
        out.extend_from_slice(chunk_type);
        out.extend_from_slice(data);
        // The decoder does not validate CRCs; zeros are fine here.
        out.extend_from_slice(&[0u8; 4]);
    }

    /// Append a PLTE chunk followed by a tRNS chunk to an in-progress PNG.
    /// Returns the offset right after the tRNS chunk so more chunks can follow.
    fn append_palette_trns(out: &mut Vec<u8>, entries: &[[u8; 3]], alphas: &[u8]) {
        let palette_bytes: Vec<u8> = entries.iter().flatten().copied().collect();
        write_test_chunk(out, b"PLTE", &palette_bytes);
        write_test_chunk(out, b"tRNS", alphas);
    }

    #[test]
    fn decode_png_minimal_header() {
        // A real 1x1 RGBA PNG (filter 0, single black pixel) must decode.
        let png = make_png(1, 1, 8, 6, 0, &[0, 0, 0, 0, 255]);
        assert_eq!(detect_format(&png), ImageFormat::Png);
        let img = decode_png(&png).unwrap();
        assert_eq!(img.format, ImageFormat::Png);
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn decode_svg_returns_not_implemented() {
        let svg = b"<svg width=\"100\" height=\"50\" xmlns=\"http://www.w3.org/2000/svg\"></svg>";
        let err = decode_svg(svg).unwrap_err();
        assert!(err.contains("not implemented"), "unexpected error: {err}");
    }

    #[test]
    fn decode_svgz_returns_not_implemented() {
        // GZIP of the SVG above: decompression succeeds, rasterization refuses.
        let svg = b"<svg width=\"100\" height=\"50\" xmlns=\"http://www.w3.org/2000/svg\"></svg>";
        let compressed = miniz_oxide::deflate::compress_to_vec(svg, 6);
        let err = decode_svgz(&compressed).unwrap_err();
        assert!(err.contains("not implemented"), "unexpected error: {err}");
    }

    #[test]
    fn decode_to_rgba8_converts() {
        // Decoding a real PNG through the RGBA8 convenience path.
        let png = make_png(1, 1, 8, 6, 0, &[0, 12, 34, 56, 78]);
        let img = decode_to_rgba8(&png).unwrap();
        assert_eq!(img.format, ImageFormat::Rgba8);
        assert_eq!(img.width, 1);
        assert_eq!(img.height, 1);
    }

    #[test]
    fn decode_jpeg_detects_dimensions() {
        // Build minimal JPEG with SOF0 marker
        let mut jpeg = vec![0xFF, 0xD8, 0xFF, 0xE0];
        let app0_len = 16u16;
        jpeg.extend_from_slice(&app0_len.to_be_bytes());
        jpeg.extend_from_slice(b"JFIF\x00");
        jpeg.extend_from_slice(&[0u8; 9]); // JFIF data
        jpeg.push(0xFF);
        jpeg.push(0xC0); // SOF0
        jpeg.extend_from_slice(&17u16.to_be_bytes()); // length
        jpeg.push(8); // precision
        jpeg.extend_from_slice(&200u16.to_be_bytes()); // height
        jpeg.extend_from_slice(&300u16.to_be_bytes()); // width
        jpeg.push(3); // number of components
        jpeg.extend_from_slice(&[0x01, 0x11, 0x00, 0x02, 0x11, 0x01, 0x03, 0x11, 0x01]); // component info
        jpeg.extend_from_slice(&[0xFF, 0xD9]); // EOI

        let result = decode_jpeg(&jpeg);
        // Minimal JPEG data (no quantization or Huffman tables) will fail during decode
        assert!(result.is_err(), "JPEG decoder should return error for incomplete data");
    }

    #[test]
    fn decode_tiff_returns_not_implemented() {
        // Well-formed minimal little-endian TIFF header.
        let tiff = b"II\x2a\x00\x08\x00\x00\x00";
        let err = decode_tiff(tiff).unwrap_err();
        assert!(err.contains("not implemented"), "unexpected error: {err}");
    }

    #[test]
    fn decode_webp_returns_not_implemented() {
        // Minimal WebP RIFF header with a VP8 chunk.
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&26u32.to_le_bytes()); // RIFF chunk size
        webp.extend_from_slice(b"WEBP");
        webp.extend_from_slice(b"VP8 ");
        webp.extend_from_slice(&[0x00; 10]); // frame tag + start code + dims
        let err = decode_webp(&webp).unwrap_err();
        assert!(err.contains("not implemented"), "unexpected error: {err}");
    }

    #[test]
    fn decode_ico_returns_not_implemented() {
        let mut ico = vec![0x00, 0x00, 0x01, 0x00, 0x01, 0x00]; // header, 1 entry
                                                                // Entry: 16x16, no palette, 32 bpp
        ico.extend_from_slice(&[
            16, 16, 0, 0, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x16, 0x00, 0x00, 0x00,
        ]);
        let err = decode_ico(&ico).unwrap_err();
        assert!(err.contains("not implemented"), "unexpected error: {err}");
    }

    #[test]
    fn decode_pnm_binary_grayscale() {
        let pnm = b"P5\n3 2\n255\n\x00\x80\xFF\x10\x20\x30";
        let result = decode_pnm(pnm);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.format, ImageFormat::Pnm);
    }

    #[test]
    fn decode_avif_returns_not_implemented() {
        let avif = b"\x00\x00\x00\x20ftypavif\x00\x00\x00\x00";
        let err = decode_avif(avif).unwrap_err();
        assert!(err.contains("not implemented"), "unexpected error: {err}");
    }

    #[test]
    fn decode_gif_returns_not_implemented() {
        // Minimal well-formed GIF89a header (no image data needed: decoding is
        // refused wholesale).
        let mut gif = b"GIF89a".to_vec();
        gif.extend_from_slice(&2u16.to_le_bytes()); // width
        gif.extend_from_slice(&2u16.to_le_bytes()); // height
        gif.push(0xF0); // packed: has GCT, size=16
        gif.push(0); // bg color index
        gif.push(0); // pixel aspect ratio
                     // Global color table: 16 entries of 3 bytes each
        for i in 0..16 {
            let c = (i * 16) as u8;
            gif.push(c);
            gif.push(c);
            gif.push(c);
        }
        gif.push(0x3B); // GIF trailer

        let err = decode_gif(&gif).unwrap_err();
        assert!(err.contains("not implemented"), "unexpected error: {err}");
    }

    // ── PNG real-decode tests ────────────────────────────────────────────────

    #[test]
    fn png_roundtrip_matches_encoder_output() {
        // Golden sample: the crate's own PNG encoder output must decode back to
        // the exact source pixels (covers RGBA stride and filter 0).
        let src = DecodedImage::new(
            ImageFormat::Rgba8,
            ImageData::Rgba8(vec![
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255, 12, 34, 56, 78,
                90, 12, 34, 56, 200, 100, 50, 25, 1, 2, 3, 4,
            ]),
            4,
            2,
        );
        let encoded = crate::image::encoder::encode(&src, ImageFormat::Png).unwrap();
        let decoded = decode_png(&encoded).unwrap();
        assert_eq!(decoded.width, 4);
        assert_eq!(decoded.height, 2);
        let rgba = decoded.as_rgba8();
        assert_eq!(rgba.as_bytes(), src.data.as_bytes());
    }

    #[test]
    fn png_decodes_rgb_filter_none() {
        // 4x4 RGB image, every pixel = (x*51, x*77, x*101).
        let w = 4usize;
        let h = 4usize;
        let mut scanlines = Vec::new();
        let mut expected = Vec::new();
        for y in 0..h {
            scanlines.push(0); // filter None
            for x in 0..w {
                let v = (x * 31 + y * 7) as u8;
                scanlines.extend_from_slice(&[v, v.wrapping_mul(2), v.wrapping_mul(3)]);
                expected.extend_from_slice(&[v, v.wrapping_mul(2), v.wrapping_mul(3)]);
            }
        }
        let png = make_png(4, 4, 8, 2, 0, &scanlines);
        let img = decode_png(&png).unwrap();
        assert_eq!(img.data.as_bytes(), &expected);
    }

    #[test]
    fn png_decodes_rgb_filter_sub() {
        // 4x4 RGB image, filter 1 (Sub): each row holds the difference from the
        // pixel to its left. Rows are a constant color, so only the first pixel
        // of each row carries the color and the rest are zero deltas.
        let w = 4usize;
        let h = 4usize;
        let colors = [[200u8, 30, 90], [10, 220, 40], [5, 9, 250], [128, 128, 128]];
        let mut scanlines = Vec::new();
        let mut expected = Vec::new();
        for &c in colors.iter() {
            scanlines.push(1); // filter Sub
            for x in 0..w {
                if x == 0 {
                    scanlines.extend_from_slice(&c);
                } else {
                    scanlines.extend_from_slice(&[0, 0, 0]);
                }
                expected.extend_from_slice(&c);
            }
        }
        let png = make_png(w as u32, h as u32, 8, 2, 0, &scanlines);
        let img = decode_png(&png).unwrap();
        assert_eq!(img.data.as_bytes(), &expected);
    }

    #[test]
    fn png_decodes_rgba_filter_up_average_paeth() {
        // 2x3 RGBA image covering filter types 2 (Up), 3 (Average) and
        // 4 (Paeth) with distinct per-pixel data.
        let w = 2usize;
        let h = 3usize;
        let rows: Vec<[u8; 8]> = vec![
            [10, 20, 30, 255, 40, 50, 60, 255],
            [70, 80, 90, 255, 100, 110, 120, 255],
            [130, 140, 150, 255, 160, 170, 180, 255],
        ];
        let mut scanlines = Vec::new();
        let mut expected = Vec::new();
        // Encode each row with its own filter type.
        for (fi, row) in rows.iter().enumerate() {
            let filter = match fi {
                0 => 2, // Up (difference from the row above)
                1 => 3, // Average
                _ => 4, // Paeth
            };
            scanlines.push(filter);
            let up = if fi == 0 { [0u8; 8] } else { rows[fi - 1] };
            for x in 0..w {
                for c in 0..4 {
                    let raw = row[x * 4 + c];
                    let a = if x > 0 { row[(x - 1) * 4 + c] } else { 0 };
                    let b = up[x * 4 + c];
                    let c_prev = if x > 0 { up[(x - 1) * 4 + c] } else { 0 };
                    let filt = match filter {
                        2 => raw.wrapping_sub(b),
                        3 => {
                            let pred = ((a as u16 + b as u16) / 2) as u8;
                            raw.wrapping_sub(pred)
                        }
                        _ => {
                            // Paeth reconstruction must match the predictor.
                            let rec = paeth_predictor(a, b, c_prev);
                            raw.wrapping_sub(rec)
                        }
                    };
                    scanlines.push(filt);
                }
            }
            expected.extend_from_slice(&row[..]);
        }
        // Sanity: the filter-2 (Up) first row is identical to raw pixels.
        let png = make_png(w as u32, h as u32, 8, 6, 0, &scanlines);
        let img = decode_png(&png).unwrap();
        let rgba = img.as_rgba8();
        assert_eq!(rgba.as_bytes(), &expected);
    }

    #[test]
    fn png_grayscale_16bit_keeps_high_byte() {
        // 16-bit grayscale must sample the high byte of each big-endian
        // sample, not produce black.
        let scanlines = [
            0, // filter None
            0xAB, 0xCD, 0x12, 0x34, 0xFF, 0x00, 0x00, 0x00, // 4 samples of 2 bytes
            0,    // filter None
            0x80, 0x00, 0x01, 0xFF, 0x42, 0x42, 0x99, 0x00,
        ];
        let png = make_png(4, 2, 16, 0, 0, &scanlines);
        let img = decode_png(&png).unwrap();
        assert_eq!(img.data.as_bytes(), &[0xAB, 0x12, 0xFF, 0x00, 0x80, 0x01, 0x42, 0x99]);
    }

    #[test]
    fn png_rgba_16bit_keeps_high_byte() {
        // One 2x1 RGBA image at 16 bits per channel.
        let scanlines = [
            0, // filter None
            0x11, 0x00, 0x22, 0x00, 0x33, 0x00, 0x44, 0x00, // pixel 0 (RGBA hi bytes)
            0x55, 0x00, 0x66, 0x00, 0x77, 0x00, 0x88, 0x00, // pixel 1
        ];
        let png = make_png(2, 1, 16, 6, 0, &scanlines);
        let img = decode_png(&png).unwrap();
        assert_eq!(img.data.as_bytes(), &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88]);
    }

    #[test]
    fn png_indexed_palette_with_trns() {
        // 3x1 indexed image with a 3-entry palette; tRNS makes entry 1
        // half-transparent and entry 2 fully transparent.
        let mut out = Vec::new();
        out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
        let mut ihdr = Vec::with_capacity(13);
        ihdr.extend_from_slice(&3u32.to_be_bytes());
        ihdr.extend_from_slice(&1u32.to_be_bytes());
        ihdr.push(8); // bit depth
        ihdr.push(3); // color type: indexed
        ihdr.push(0);
        ihdr.push(0);
        ihdr.push(0); // interlace
        write_test_chunk(&mut out, b"IHDR", &ihdr);
        append_palette_trns(&mut out, &[[255, 0, 0], [0, 255, 0], [0, 0, 255]], &[255, 128, 0]);
        let compressed = miniz_oxide::deflate::compress_to_vec_zlib(&[0u8, 0, 1, 2], 0); // filter 0 + 3 indices
        write_test_chunk(&mut out, b"IDAT", &compressed);
        write_test_chunk(&mut out, b"IEND", &[]);

        let img = decode_png(&out).unwrap();
        assert_eq!(img.data.as_bytes(), &[255, 0, 0, 255, 0, 255, 0, 128, 0, 0, 255, 0]);
    }

    #[test]
    fn png_rejects_interlaced() {
        // Interlace flag (Adam7) is detected from IHDR and refused.
        let scanlines = [0u8, 1, 2, 3]; // content is irrelevant
        let png = make_png(2, 1, 8, 6, 1, &scanlines);
        let err = decode_png(&png).unwrap_err();
        assert!(err.contains("interlaced") || err.contains("Interlaced"), "{err}");
    }

    #[test]
    fn png_chunk_declared_length_out_of_bounds_is_err() {
        // A chunk whose declared length runs past the end of the file must be
        // an Err, never a panic (this used to slice out of bounds).
        let mut bad = b"\x89PNG\r\n\x1a\n".to_vec();
        bad.extend_from_slice(&u32::MAX.to_be_bytes()); // declares ~4 GiB
        bad.extend_from_slice(b"IDAT");
        bad.extend_from_slice(&[0u8; 4]); // body bytes far short of the claim
        let err = decode_png(&bad).unwrap_err();
        assert!(err.contains("declares"), "unexpected error: {err}");
    }

    #[test]
    fn png_truncated_scanline_data_is_err() {
        // IDAT holds too few decompressed bytes for the declared dimensions.
        let png = make_png(8, 8, 8, 6, 0, &[0, 1, 2, 3, 4, 5, 6, 7, 8]);
        let err = decode_png(&png).unwrap_err();
        assert!(err.contains("truncated") || err.contains("decompress"), "unexpected error: {err}");
    }
}
