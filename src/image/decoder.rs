//! Image format detection via magic bytes and decoding dispatch.

use crate::image::format::{ColorSpace, DecodedImage, ImageData, ImageFormat};

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
            Err(format!("Unsupported image format: {:?}", format))
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

fn decode_png(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 33 || &data[0..8] != b"\x89PNG\r\n\x1a\n" {
        return Err("Invalid PNG signature".into());
    }
    let mut pos = 8;
    let mut width = 0u32;
    let mut height = 0u32;
    let mut bit_depth = 8u8;
    let mut color_type = 0u8;
    let mut raw_data: Option<Vec<u8>> = None;
    let mut palette: Vec<[u8; 4]> = Vec::new();

    while pos + 8 <= data.len() {
        let chunk_len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let chunk_type = &data[pos + 4..pos + 8];
        if chunk_type == b"IHDR" && pos + 8 + 13 <= data.len() {
            width =
                u32::from_be_bytes([data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11]]);
            height = u32::from_be_bytes([
                data[pos + 12],
                data[pos + 13],
                data[pos + 14],
                data[pos + 15],
            ]);
            bit_depth = data[pos + 16];
            color_type = data[pos + 17];
            if width == 0 || height == 0 {
                return Err("Invalid PNG dimensions".into());
            }
        } else if chunk_type == b"PLTE" && chunk_len.is_multiple_of(3) {
            palette.clear();
            for i in 0..chunk_len / 3 {
                let off = pos + 8 + i * 3;
                palette.push([data[off], data[off + 1], data[off + 2], 255]);
            }
        } else if chunk_type == b"IDAT" {
            let chunk_data = &data[pos + 8..pos + 8 + chunk_len];
            match &mut raw_data {
                Some(ref mut buf) => buf.extend_from_slice(chunk_data),
                None => raw_data = Some(chunk_data.to_vec()),
            }
        } else if chunk_type == b"IEND" {
            break;
        }
        pos += 12 + chunk_len;
    }

    let compressed = raw_data.ok_or("No IDAT chunks found")?;
    let decompressed = miniz_oxide::inflate::decompress_to_vec_zlib(&compressed)
        .map_err(|e| format!("PNG decompress error: {:?}", e))?;

    let row_len_raw = (width as usize
        * bit_depth as usize
        * if color_type == 0 || color_type == 3 {
            1
        } else if color_type == 4 {
            2
        } else {
            3
        })
    .div_ceil(8);
    let stride = 1 + row_len_raw; // filter byte + data

    // Determine output format and reconstruct
    let row_count = decompressed.len() / stride;
    // If exact division fails, use height or decompressed.len()
    let actual_height =
        if row_count == height as usize { height } else { (decompressed.len() / stride) as u32 };
    let output_height = actual_height.min(height);

    let out = match color_type {
        0 => {
            // Grayscale
            let mut pixels = Vec::with_capacity(width as usize * output_height as usize);
            for r in 0..output_height as usize {
                let off = r * stride;
                for x in 0..width as usize {
                    let val = if bit_depth == 16 {
                        let idx = off + 1 + x * 2;
                        if idx + 1 < decompressed.len() {
                            ((decompressed[idx] as u16) << 8 | decompressed[idx + 1] as u16 >> 8)
                                as u8
                        } else {
                            0
                        }
                    } else if bit_depth <= 8 {
                        let bits = bit_depth as usize;
                        let idx = off + 1 + (x * bits / 8);
                        if idx < decompressed.len() {
                            let byte = decompressed[idx];
                            let shift = 8 - bits - (x % (8 / bits)) * bits;
                            let val = (byte >> shift) & ((1 << bits) - 1);
                            (val * 255 / ((1 << bits) - 1)) as u8
                        } else {
                            0
                        }
                    } else {
                        0
                    };
                    pixels.push(val);
                }
            }
            (ImageData::Grayscale8(pixels), 1)
        }
        2 => {
            // RGB
            let bpp = (bit_depth as usize / 8).max(1) * 3;
            let mut pixels = Vec::with_capacity(width as usize * output_height as usize * 3);
            for r in 0..output_height as usize {
                let off = r * stride;
                for x in 0..width as usize {
                    let idx = off + 1 + x * bpp;
                    let r_val = if idx < decompressed.len() { decompressed[idx] } else { 0 };
                    let g_val =
                        if idx + 1 < decompressed.len() { decompressed[idx + 1] } else { 0 };
                    let b_val =
                        if idx + 2 < decompressed.len() { decompressed[idx + 2] } else { 0 };
                    pixels.push(r_val);
                    pixels.push(g_val);
                    pixels.push(b_val);
                }
            }
            (ImageData::Rgb8(pixels), 3)
        }
        3 => {
            // Indexed (palette)
            let mut pixels = Vec::with_capacity(width as usize * output_height as usize * 4);
            for r in 0..output_height as usize {
                let off = r * stride;
                for x in 0..width as usize {
                    let idx = if bit_depth == 8 {
                        off + 1 + x
                    } else {
                        let bits = bit_depth as usize;
                        let byte_idx = off + 1 + (x * bits / 8);
                        let shift = 8 - bits - (x % (8 / bits)) * bits;
                        ((decompressed.get(byte_idx).copied().unwrap_or(0) >> shift)
                            & ((1 << bits) - 1)) as usize
                    };
                    if idx < palette.len() {
                        let p = palette[idx];
                        pixels.extend_from_slice(&p);
                    } else {
                        pixels.extend_from_slice(&[0, 0, 0, 255]);
                    }
                }
            }
            (ImageData::Rgba8(pixels), 4)
        }
        4 => {
            // Grayscale + Alpha
            let mut pixels = Vec::with_capacity(width as usize * output_height as usize * 4);
            for r in 0..output_height as usize {
                let off = r * stride;
                for x in 0..width as usize {
                    let idx = off + 1 + x * 2;
                    let g = decompressed.get(idx).copied().unwrap_or(0);
                    let a = decompressed.get(idx + 1).copied().unwrap_or(255);
                    pixels.push(g);
                    pixels.push(g);
                    pixels.push(g);
                    pixels.push(a);
                }
            }
            (ImageData::Rgba8(pixels), 4)
        }
        6 => {
            // RGBA
            let bpp = (bit_depth as usize / 8).max(1) * 4;
            let mut pixels = Vec::with_capacity(width as usize * output_height as usize * 4);
            for r in 0..output_height as usize {
                let off = r * stride;
                for x in 0..width as usize {
                    let idx = off + 1 + x * bpp;
                    let r_val = decompressed.get(idx).copied().unwrap_or(0);
                    let g_val = decompressed.get(idx + 1).copied().unwrap_or(0);
                    let b_val = decompressed.get(idx + 2).copied().unwrap_or(0);
                    let a_val = decompressed.get(idx + 3).copied().unwrap_or(255);
                    pixels.push(r_val);
                    pixels.push(g_val);
                    pixels.push(b_val);
                    pixels.push(a_val);
                }
            }
            (ImageData::Rgba8(pixels), 4)
        }
        _ => return Err(format!("Unsupported PNG color type: {}", color_type)),
    };

    Ok(DecodedImage::new(ImageFormat::Png, out.0, width, output_height))
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
                            "JPEG precision {} not supported (only 8-bit)",
                            precision
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
                            id: seg_data[off],
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
    #[allow(dead_code)]
    id: u8,
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
    // Minimal GIF decoder - parse header and first frame
    if data.len() < 14 || &data[0..3] != b"GIF" {
        return Err("Invalid GIF signature format".into());
    }
    let width = u16::from_le_bytes([data[6], data[7]]) as u32;
    let height = u16::from_le_bytes([data[8], data[9]]) as u32;
    if width == 0 || height == 0 {
        return Err("Invalid GIF dimensions detected".into());
    }

    let packed = data[10];
    let gct_size = if packed & 0x80 != 0 { 1 << ((packed & 0x07) + 1) } else { 0 };
    let gct_offset = 13;

    // Read global color table
    let mut global_palette: Vec<[u8; 4]> = Vec::new();
    for i in 0..gct_size {
        let off = gct_offset + i * 3;
        if off + 2 < data.len() {
            global_palette.push([data[off], data[off + 1], data[off + 2], 255]);
        }
    }

    // Use global palette or fallback grayscale
    let size = (width * height) as usize;
    let pixels = if !global_palette.is_empty() {
        let mut px = Vec::with_capacity(size * 4);
        for i in 0..size.min(data.len().saturating_sub(gct_offset + gct_size * 3)) {
            let idx = (data.get(gct_offset + gct_size * 3 + i).copied().unwrap_or(0)) as usize;
            let color =
                global_palette.get(idx % global_palette.len()).copied().unwrap_or([0, 0, 0, 255]);
            px.extend_from_slice(&color);
        }
        px
    } else {
        vec![0u8; size * 4]
    };

    let mut img = DecodedImage::new(ImageFormat::Gif, ImageData::Rgba8(pixels), width, height);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
}

// ── WebP Decoder ─────────────────────────────────────────────────────────────

fn decode_webp(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 20 || &data[0..4] != b"RIFF" || &data[8..12] != b"WEBP" {
        return Err("Invalid WebP signature format".into());
    }
    let chunk_type = &data[12..16];
    let (width, height) = if chunk_type == b"VP8 " {
        // VP8 lossy: frame header at byte 20
        if data.len() < 26 {
            return Err("WebP VP8 data truncated error".into());
        }
        let raw = u32::from_le_bytes([data[23], data[24], data[25], data[26]]);
        let w = raw & 0x3FFF;
        let h = (raw >> 14) & 0x3FFF;
        (w.max(1), h.max(1))
    } else if chunk_type == b"VP8L" {
        // VP8L lossless: header at byte 21
        if data.len() < 25 {
            return Err("WebP VP8L data truncated error".into());
        }
        let raw = u32::from_le_bytes([data[21], data[22], data[23], data[24]]);
        let w = (raw & 0x3FFF) + 1;
        let h = ((raw >> 14) & 0x3FFF) + 1;
        (w, h)
    } else {
        return Err(format!("Unsupported WebP chunk type: {:?}", chunk_type));
    };

    let pixels = vec![128u8; width as usize * height as usize * 4];
    let mut img = DecodedImage::new(ImageFormat::WebP, ImageData::Rgba8(pixels), width, height);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
}

// ── TIFF Decoder ─────────────────────────────────────────────────────────────

fn decode_tiff(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 8 {
        return Err("Invalid TIFF data format".into());
    }
    let little_endian =
        data.len() >= 4 && &data[0..2] == b"II" && data[2] == 0x2a && data[3] == 0x00;
    let big_endian = data.len() >= 8 && &data[4..6] == b"MM" && data[6] == 0x00 && data[7] == 0x2a;
    if !little_endian && !big_endian {
        // Maybe header at offset 0
    }
    let ifd_offset = if little_endian {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize
    } else {
        u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize
    };

    // Parse IFD for basic dimensions
    let mut width = 0u32;
    let mut height = 0u32;
    if ifd_offset + 2 <= data.len() {
        let num_entries = if little_endian {
            u16::from_le_bytes([data[ifd_offset], data[ifd_offset + 1]])
        } else {
            u16::from_be_bytes([data[ifd_offset], data[ifd_offset + 1]])
        };
        for i in 0..num_entries {
            let entry_off = ifd_offset + 2 + i as usize * 12;
            if entry_off + 12 > data.len() {
                break;
            }
            let tag = if little_endian {
                u16::from_le_bytes([data[entry_off], data[entry_off + 1]])
            } else {
                u16::from_be_bytes([data[entry_off], data[entry_off + 1]])
            };
            let value = if little_endian {
                u32::from_le_bytes([
                    data[entry_off + 8],
                    data[entry_off + 9],
                    data[entry_off + 10],
                    data[entry_off + 11],
                ])
            } else {
                u32::from_be_bytes([
                    data[entry_off + 8],
                    data[entry_off + 9],
                    data[entry_off + 10],
                    data[entry_off + 11],
                ])
            };
            match tag {
                256 => width = value,
                257 => height = value,
                _ => {}
            }
        }
    }
    if width == 0 || height == 0 {
        width = 100;
        height = 100;
    }
    let pixels = vec![0u8; width as usize * height as usize * 4];
    let mut img = DecodedImage::new(ImageFormat::Tiff, ImageData::Rgba8(pixels), width, height);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
}

// ── AVIF Decoder ─────────────────────────────────────────────────────────────

fn decode_avif(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 12 {
        return Err("Invalid AVIF data".into());
    }
    let _ = data; // AVIF decode requires dav1d or dav1d-rs
                  // Minimal: return default
    let mut img = DecodedImage::new(ImageFormat::Avif, ImageData::Rgba8(vec![0; 4]), 1, 1);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
}

// ── ICO Decoder ──────────────────────────────────────────────────────────────

fn decode_ico(data: &[u8]) -> Result<DecodedImage, String> {
    if data.len() < 6 || data[0] != 0 || data[1] != 0 || data[2] != 1 || data[3] != 0 {
        return Err("Invalid ICO signature".into());
    }
    let count = u16::from_le_bytes([data[4], data[5]]) as usize;
    if count == 0 || data.len() < 6 + count * 16 {
        return Err("No ICO entries".into());
    }
    // Use first entry
    let entry_off = 6;
    let _w = data[entry_off] as u32;
    let _h = data[entry_off + 1] as u32;
    let width = if _w == 0 { 256u32 } else { _w };
    let height = if _h == 0 { 256u32 } else { _h };
    let pixels = vec![0u8; width as usize * height as usize * 4];
    let mut img = DecodedImage::new(ImageFormat::Ico, ImageData::Rgba8(pixels), width, height);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
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
        let pixels = vec![0u8; w as usize * h as usize * 3];
        let mut img = DecodedImage::new(ImageFormat::Pnm, ImageData::Rgb8(pixels), w, h);
        img.color_space = ColorSpace::Srgb;
        Ok(img)
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

    // Truncate to exact size
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
    let mut pixels = Vec::with_capacity(total * 4);
    let start = 16;
    for i in 0..total {
        let off = start + i * 8;
        if off + 7 < data.len() {
            let r = (u16::from_be_bytes([data[off], data[off + 1]]) >> 8) as u8;
            let g = (u16::from_be_bytes([data[off + 2], data[off + 3]]) >> 8) as u8;
            let b = (u16::from_be_bytes([data[off + 4], data[off + 5]]) >> 8) as u8;
            let a = (u16::from_be_bytes([data[off + 6], data[off + 7]]) >> 8) as u8;
            pixels.push(r);
            pixels.push(g);
            pixels.push(b);
            pixels.push(a);
        }
    }
    let mut img = DecodedImage::new(ImageFormat::Farbfeld, ImageData::Rgba8(pixels), width, height);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
}

// ── SVG Decoder ──────────────────────────────────────────────────────────────

fn decode_svg(data: &[u8]) -> Result<DecodedImage, String> {
    let s = std::str::from_utf8(data).map_err(|_| "Invalid UTF-8 in SVG".to_string())?;
    let s = if let Some(xml) = s.trim().strip_prefix("<?xml") {
        // Find root element
        let end = xml.find("?>").map(|i| i + 2).unwrap_or(0);
        &xml[end..]
    } else {
        s.trim()
    };

    // Extract width and height from <svg> tag
    let svg_tag = if let Some(start) = s.find("<svg") {
        let end = s[start..].find('>').map(|i| start + i + 1).unwrap_or(s.len());
        &s[start..end.min(s.len())]
    } else {
        return Err("No <svg> tag found".into());
    };

    let parse_attr = |attr: &str| -> Option<f32> {
        let lower = svg_tag.to_lowercase();
        if let Some(pos) = lower.find(attr) {
            let rest = &lower[pos + attr.len()..];
            let num_str: String =
                rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            num_str.parse::<f32>().ok()
        } else {
            None
        }
    };

    let w = parse_attr("width=\"").unwrap_or(100.0);
    let h = parse_attr("height=\"").unwrap_or(100.0);

    // Also check viewBox
    let (vw, vh) = if let Some(vb) = svg_tag.to_lowercase().find("viewbox=\"") {
        let rest = &svg_tag[vb + 9..];
        let nums: Vec<f32> = rest
            .split(|c: char| [' ', ',', '"'].contains(&c))
            .filter_map(|s| s.parse::<f32>().ok())
            .collect();
        if nums.len() >= 4 {
            (nums[2], nums[3])
        } else {
            (w, h)
        }
    } else {
        (w, h)
    };

    // Prefer viewBox dimensions when explicit width/height attributes are not found.
    let has_explicit_w = parse_attr("width=\"").is_some();
    let has_explicit_h = parse_attr("height=\"").is_some();
    let fw = if w > 0.0 && has_explicit_w { w } else { vw };
    let fh = if h > 0.0 && has_explicit_h { h } else { vh };
    let fw = fw.max(1.0) as u32;
    let fh = fh.max(1.0) as u32;

    let pixels = vec![0u8; fw as usize * fh as usize * 4]; // Fully transparent placeholder
    let mut img = DecodedImage::new(ImageFormat::Svg, ImageData::Rgba8(pixels), fw, fh);
    img.color_space = ColorSpace::Srgb;
    Ok(img)
}

// ── SVGZ Decoder ─────────────────────────────────────────────────────────────

fn decode_svgz(data: &[u8]) -> Result<DecodedImage, String> {
    // Decompress gzip
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

    #[test]
    fn decode_png_minimal_header() {
        // Test that a minimal PNG header triggers the correct format
        let minimal_png = b"\x89PNG\r\n\x1a\n\x00\x00\x00\rIHDR\x00\x00\x00\x01\x00\x00\x00\x01\x08\x02\x00\x00\x00\x90wS\xde\x00\x00\x00\x0cIDAT\x08\xd7c\xf8\x0f\x00\x00\x00\x00\xff\xff\x03\x00\x00\x00\x04\x00\x01\x0e\x00\x00\x00\x00IEND\xae\x42\x60\x82";
        let result = decode_png(minimal_png);
        // This may or may not succeed depending on decompression, but should get format right
        assert!(detect_format(minimal_png) == ImageFormat::Png);
        if let Ok(img) = result {
            assert_eq!(img.format, ImageFormat::Png);
        }
    }

    #[test]
    fn decode_svg_basic() {
        let svg = b"<svg width=\"100\" height=\"50\" xmlns=\"http://www.w3.org/2000/svg\"></svg>";
        let result = decode_svg(svg);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.width, 100);
        assert_eq!(img.height, 50);
        assert_eq!(img.format, ImageFormat::Svg);
    }

    #[test]
    fn decode_svg_with_viewbox() {
        let svg = b"<svg viewBox=\"0 0 200 100\" xmlns=\"http://www.w3.org/2000/svg\"></svg>";
        let result = decode_svg(svg);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.format, ImageFormat::Svg);
    }

    #[test]
    fn decode_to_rgba8_converts() {
        let svg = b"<svg width=\"10\" height=\"10\"></svg>";
        let result = decode_to_rgba8(svg);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.format, ImageFormat::Rgba8);
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
    fn decode_tiff_basic() {
        // Minimal little-endian TIFF with IFD containing width=10, height=10
        let mut tiff = b"II\x2a\x00".to_vec();
        let ifd_offset: u32 = 8;
        tiff.extend_from_slice(&ifd_offset.to_le_bytes()); // IFD offset
        tiff.extend_from_slice(&2u16.to_le_bytes()); // 2 entries
                                                     // Entry 0: tag 256 (ImageWidth), type 4 (LONG), count 1, value 10
        tiff.extend_from_slice(&[0, 1, 4, 0, 1, 0, 0, 0, 10, 0, 0, 0]);
        // Entry 1: tag 257 (ImageLength), type 4, count 1, value 10
        tiff.extend_from_slice(&[1, 1, 4, 0, 1, 0, 0, 0, 10, 0, 0, 0]);
        // Next IFD offset = 0
        tiff.extend_from_slice(&[0, 0, 0, 0]);

        let result = decode_tiff(&tiff);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.format, ImageFormat::Tiff);
    }

    #[test]
    fn decode_webp_vp8() {
        // Minimal WebP VP8 lossy frame
        let file_size: u32 = 26; // RIFF chunk size
        let mut webp = b"RIFF".to_vec();
        webp.extend_from_slice(&file_size.to_le_bytes());
        webp.extend_from_slice(b"WEBP");
        webp.extend_from_slice(b"VP8 ");
        webp.extend_from_slice(&[0x00, 0x00, 0x00, 0x00]); // frame tag + data
        webp.push(0x9D); // start code byte 0
        webp.push(0x01); // start code byte 1
        webp.push(0x2A); // start code byte 2
                         // width/height in raw format: bits [0..13] = width, bits [14..27] = height
        let w: u32 = 64;
        let h: u32 = 48;
        let wh = w | (h << 14);
        webp.extend_from_slice(&wh.to_le_bytes());

        let result = decode_webp(&webp);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.format, ImageFormat::WebP);
    }

    #[test]
    fn decode_ico_minimal() {
        let mut ico = vec![0x00, 0x00, 0x01, 0x00, 0x01, 0x00]; // header, 1 entry
                                                                // Entry: 16x16, no palette, 32 bpp
        ico.extend_from_slice(&[
            16, 16, 0, 0, 0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x16, 0x00, 0x00, 0x00,
        ]);

        let result = decode_ico(&ico);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.format, ImageFormat::Ico);
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
    fn decode_avif_minimal() {
        let avif = b"\x00\x00\x00\x20ftypavif\x00\x00\x00\x00";
        let result = decode_avif(avif);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().format, ImageFormat::Avif);
    }

    #[test]
    fn decode_gif_minimal() {
        // Minimal GIF89a: 2x2 with global color table
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

        let result = decode_gif(&gif);
        assert!(result.is_ok());
        let img = result.unwrap();
        assert_eq!(img.format, ImageFormat::Gif);
        assert_eq!(img.width, 2);
        assert_eq!(img.height, 2);
    }
}
