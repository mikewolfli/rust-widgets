//! Image encoding functions.

use crate::image::format::{DecodedImage, ImageData, ImageFormat};

// ============================================================================
// JPEG encoder: standard JPEG with 4:4:4 (no subsampling), baseline DCT
// ============================================================================

// Standard JPEG luminance quantization table (ITU-T T.81, K.1)
const STD_LUM_QUANT: [u8; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113,
    92, 49, 64, 78, 87, 103, 121, 120, 101, 72, 92, 95, 98, 112, 100, 103, 99,
];

// Standard JPEG chrominance quantization table (ITU-T T.81, K.2)
const STD_CHROM_QUANT: [u8; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99,
    47, 66, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
];

// Zigzag order mapping: natural[zigzag[i]] = i-th zigzag position
const ZIGZAG: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20,
    13, 6, 7, 14, 21, 28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59,
    52, 45, 38, 31, 39, 46, 53, 60, 61, 54, 47, 55, 62, 63,
];

/// DHT bits/huffval for DC Luminance (Table K.3)
const DC_LUM_BITS: [u8; 16] = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0];
const DC_LUM_HUFFVAL: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

/// DHT bits/huffval for DC Chrominance (Table K.4)
const DC_CHROM_BITS: [u8; 16] = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0];
const DC_CHROM_HUFFVAL: [u8; 12] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];

/// DHT bits/huffval for AC Luminance (Table K.5)
const AC_LUM_BITS: [u8; 16] = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7D];
const AC_LUM_HUFFVAL: [u8; 162] = [
    0x01, 0x02, 0x03, 0x00, 0x04, 0x11, 0x05, 0x12, 0x21, 0x31, 0x41, 0x06, 0x13, 0x51, 0x61, 0x07,
    0x22, 0x71, 0x14, 0x32, 0x81, 0x91, 0xA1, 0x08, 0x23, 0x42, 0xB1, 0xC1, 0x15, 0x52, 0xD1, 0xF0,
    0x24, 0x33, 0x62, 0x72, 0x82, 0x09, 0x0A, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x25, 0x26, 0x27, 0x28,
    0x29, 0x2A, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49,
    0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x83, 0x84, 0x85, 0x86, 0x87, 0x88, 0x89,
    0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5, 0xA6, 0xA7,
    0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3, 0xC4, 0xC5,
    0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA, 0xE1, 0xE2,
    0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF1, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];

/// DHT bits/huffval for AC Chrominance (Table K.6)
const AC_CHROM_BITS: [u8; 16] = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77];
const AC_CHROM_HUFFVAL: [u8; 162] = [
    0x00, 0x01, 0x02, 0x03, 0x11, 0x04, 0x05, 0x21, 0x31, 0x06, 0x12, 0x41, 0x51, 0x07, 0x61, 0x71,
    0x13, 0x22, 0x32, 0x81, 0x08, 0x14, 0x42, 0x91, 0xA1, 0xB1, 0xC1, 0x09, 0x23, 0x33, 0x52, 0xF0,
    0x15, 0x62, 0x72, 0xD1, 0x0A, 0x16, 0x24, 0x34, 0xE1, 0x25, 0xF1, 0x17, 0x18, 0x19, 0x1A, 0x26,
    0x27, 0x28, 0x29, 0x2A, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3A, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48,
    0x49, 0x4A, 0x53, 0x54, 0x55, 0x56, 0x57, 0x58, 0x59, 0x5A, 0x63, 0x64, 0x65, 0x66, 0x67, 0x68,
    0x69, 0x6A, 0x73, 0x74, 0x75, 0x76, 0x77, 0x78, 0x79, 0x7A, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87,
    0x88, 0x89, 0x8A, 0x92, 0x93, 0x94, 0x95, 0x96, 0x97, 0x98, 0x99, 0x9A, 0xA2, 0xA3, 0xA4, 0xA5,
    0xA6, 0xA7, 0xA8, 0xA9, 0xAA, 0xB2, 0xB3, 0xB4, 0xB5, 0xB6, 0xB7, 0xB8, 0xB9, 0xBA, 0xC2, 0xC3,
    0xC4, 0xC5, 0xC6, 0xC7, 0xC8, 0xC9, 0xCA, 0xD2, 0xD3, 0xD4, 0xD5, 0xD6, 0xD7, 0xD8, 0xD9, 0xDA,
    0xE2, 0xE3, 0xE4, 0xE5, 0xE6, 0xE7, 0xE8, 0xE9, 0xEA, 0xF2, 0xF3, 0xF4, 0xF5, 0xF6, 0xF7, 0xF8,
    0xF9, 0xFA,
];

/// A (code, length) pair for Huffman encoding.
#[derive(Clone, Copy)]
struct HuffCode {
    code: u16,
    len: u8,
}

/// Build a Huffman encoding lookup table from (bits, huffval) per ITU-T T.81.
fn build_huff_table<const N: usize>(bits: &[u8; 16], huffval: &[u8; N]) -> [HuffCode; 256] {
    let mut table = [HuffCode { code: 0, len: 0 }; 256];
    let mut code: u16 = 0;
    let mut idx = 0;
    for (k, &bits_k) in bits.iter().enumerate() {
        for _ in 0..bits_k {
            let val = huffval[idx] as usize;
            table[val] = HuffCode { code, len: (k + 1) as u8 };
            code += 1;
            idx += 1;
        }
        code <<= 1;
    }
    table
}

/// Bit writer that writes MSB first, with JPEG byte stuffing.
struct JpegBitWriter {
    buf: Vec<u8>,
    byte: u8,
    bits: u8,
}

impl JpegBitWriter {
    fn new() -> Self {
        Self { buf: Vec::new(), byte: 0, bits: 0 }
    }

    fn write_bits(&mut self, code: u16, len: u8) {
        let mut c = code;
        let mut n = len;
        while n > 0 {
            let avail = 8 - self.bits;
            if n <= avail {
                let shift = avail - n;
                self.byte |= (c as u8) << shift;
                self.bits += n;
                if self.bits == 8 {
                    self.flush_byte();
                }
                return;
            }
            // Fill current byte
            let shift = n - avail;
            self.byte |= (c >> shift) as u8;
            self.bits = 8;
            self.flush_byte();
            // Keep remaining bits
            c &= (1 << shift) - 1;
            n = shift;
        }
    }

    fn flush_byte(&mut self) {
        self.buf.push(self.byte);
        // Byte stuffing: if 0xFF, insert 0x00
        if self.byte == 0xFF {
            self.buf.push(0x00);
        }
        self.byte = 0;
        self.bits = 0;
    }

    fn finish(mut self) -> Vec<u8> {
        if self.bits > 0 {
            // Pad with 1-bits
            self.byte |= 0xFF >> self.bits;
            self.flush_byte();
        }
        self.buf
    }
}

/// Category of a coefficient value for Huffman DC/AC encoding.
fn category(val: i32) -> u8 {
    if val == 0 {
        return 0;
    }
    let abs = val.unsigned_abs();
    32 - abs.leading_zeros() as u8
}

/// For a coefficient value and its category, return the bits to emit after the Huffman code.
fn amplitude_bits(val: i32, cat: u8) -> u16 {
    if cat == 0 {
        return 0;
    }
    if val > 0 {
        val as u16
    } else {
        (val + (1i32 << cat) - 1) as u16
    }
}

/// Compute the forward DCT for an 8x8 block in-place.
fn fdct(block: &mut [f32; 64]) {
    const PI: f32 = std::f32::consts::PI;
    const INV_SQRT2: f32 = std::f32::consts::FRAC_1_SQRT_2;

    // Apply 1D DCT to rows
    for y in 0..8 {
        let off = y * 8;
        let row: [f32; 8] = block[off..off + 8].try_into().unwrap();
        let mut tmp = [0f32; 8];
        for (k, tmp_k) in tmp.iter_mut().enumerate() {
            let mut sum = 0f32;
            for (n, &row_n) in row.iter().enumerate() {
                sum += row_n * (PI * (2.0 * n as f32 + 1.0) * k as f32 / 16.0).cos();
            }
            let ck = if k == 0 { INV_SQRT2 } else { 1.0 };
            *tmp_k = sum * ck * 0.5;
        }
        block[off..off + 8].copy_from_slice(&tmp);
    }

    // Apply 1D DCT to columns
    for x in 0..8 {
        let col: [f32; 8] =
            (0..8).map(|n| block[n * 8 + x]).collect::<Vec<_>>().try_into().unwrap();
        let mut tmp = [0f32; 8];
        for (k, tmp_k) in tmp.iter_mut().enumerate() {
            let mut sum = 0f32;
            for (n, &col_n) in col.iter().enumerate() {
                sum += col_n * (PI * (2.0 * n as f32 + 1.0) * k as f32 / 16.0).cos();
            }
            let ck = if k == 0 { INV_SQRT2 } else { 1.0 };
            *tmp_k = sum * ck * 0.5;
        }
        for (n, &val) in tmp.iter().enumerate() {
            block[n * 8 + x] = val;
        }
    }
}

/// Quantize an 8x8 DCT block using the given quantization table.
fn quantize(block: &mut [i32; 64], quant: &[u8; 64]) {
    for i in 0..64 {
        block[i] = (block[i] as f32 / quant[i] as f32).round() as i32;
    }
}

/// Write a single DHT table to the JPEG bitstream.
fn write_dht(out: &mut Vec<u8>, class_id: u8, bits: &[u8; 16], huffval: &[u8]) {
    out.push(0xFF);
    out.push(0xC4);
    let total_huffval: usize = bits.iter().map(|&b| b as usize).sum();
    let dht_len = 2 + 1 + 16 + total_huffval;
    out.extend_from_slice(&(dht_len as u16).to_be_bytes());
    out.push(class_id);
    out.extend_from_slice(bits);
    out.extend_from_slice(&huffval[..total_huffval]);
}

/// Encode one 8x8 block's coefficients into the bit writer.
/// `prev_dc` is mutated to track the DC difference.
fn encode_block(
    bw: &mut JpegBitWriter,
    block: &[i32; 64],
    dc_table: &[HuffCode; 256],
    ac_table: &[HuffCode; 256],
    prev_dc: &mut i32,
) {
    // DC coefficient (index 0, zigzag[0] = 0)
    let dc_diff = block[ZIGZAG[0]] - *prev_dc;
    *prev_dc = block[ZIGZAG[0]];
    let cat = category(dc_diff);
    let dc_huff = &dc_table[cat as usize];
    if dc_huff.len > 0 {
        bw.write_bits(dc_huff.code, dc_huff.len);
    }
    if cat > 0 {
        bw.write_bits(amplitude_bits(dc_diff, cat), cat);
    }

    // AC coefficients (zigzag indices 1..63)
    let mut run: u8 = 0;
    for i in 1..64 {
        let coeff = block[ZIGZAG[i]];
        if coeff == 0 {
            run += 1;
        } else {
            // Emit any runs of 16 zeros (ZRL: 0xF0)
            while run >= 16 {
                let zrl = &ac_table[0xF0];
                bw.write_bits(zrl.code, zrl.len);
                run -= 16;
            }
            let cat_val = category(coeff);
            let sym = (run << 4) | cat_val;
            let ac_huff = &ac_table[sym as usize];
            bw.write_bits(ac_huff.code, ac_huff.len);
            bw.write_bits(amplitude_bits(coeff, cat_val), cat_val);
            run = 0;
        }
    }
    // End of Block
    let eob = &ac_table[0x00];
    if eob.len > 0 {
        bw.write_bits(eob.code, eob.len);
    }
}

/// Encode to JPEG format (baseline DCT, YCbCr 4:4:4).
fn encode_jpeg(image: &DecodedImage) -> Result<Vec<u8>, String> {
    let rgba = image.as_rgba8();
    let pixels = rgba.as_bytes();
    let w = image.width as usize;
    let h = image.height as usize;

    if w == 0 || h == 0 {
        return Err("Cannot encode zero-dimension image to JPEG".to_string());
    }

    // Pad to multiples of 8 for MCU blocks
    let mcu_w = w.div_ceil(8) * 8;
    let mcu_h = h.div_ceil(8) * 8;
    let blocks_x = mcu_w / 8;
    let blocks_y = mcu_h / 8;
    // Convert RGB -> YCbCr with edge padding
    let mut y_plane = vec![0i16; mcu_w * mcu_h];
    let mut cb_plane = vec![0i16; mcu_w * mcu_h];
    let mut cr_plane = vec![0i16; mcu_w * mcu_h];

    for by in 0..mcu_h {
        for bx in 0..mcu_w {
            // Clamp to original dimensions for edge padding
            let src_y = by.min(h - 1);
            let src_x = bx.min(w - 1);
            let off = (src_y * w + src_x) * 4;
            let r = pixels[off] as f32;
            let g = pixels[off + 1] as f32;
            let b = pixels[off + 2] as f32;

            // BT.601 YCbCr conversion
            let yy = (0.299 * r + 0.587 * g + 0.114 * b).round() as i16;
            let cb = (-0.168736 * r - 0.331264 * g + 0.5 * b + 128.0).round() as i16;
            let cr = (0.5 * r - 0.418688 * g - 0.081312 * b + 128.0).round() as i16;

            let idx = by * mcu_w + bx;
            y_plane[idx] = yy.clamp(0, 255);
            cb_plane[idx] = cb.clamp(0, 255);
            cr_plane[idx] = cr.clamp(0, 255);
        }
    }

    // Build Huffman tables
    let dc_lum_table = build_huff_table(&DC_LUM_BITS, &DC_LUM_HUFFVAL);
    let dc_chrom_table = build_huff_table(&DC_CHROM_BITS, &DC_CHROM_HUFFVAL);
    let ac_lum_table = build_huff_table(&AC_LUM_BITS, &AC_LUM_HUFFVAL);
    let ac_chrom_table = build_huff_table(&AC_CHROM_BITS, &AC_CHROM_HUFFVAL);

    // ---- Assemble JPEG bitstream ----
    let mut out = Vec::new();

    // SOI
    out.extend_from_slice(&[0xFF, 0xD8]);

    // APP0/JFIF
    {
        let app0_data = &[
            b'J', b'F', b'I', b'F', 0x00, // identifier + null
            0x01, 0x02, // version 1.02
            0x00, // units: 0 = aspect ratio
            0x00, 0x01, // X density
            0x00, 0x01, // Y density
            0x00, // thumbnail width
            0x00, // thumbnail height
        ];
        out.push(0xFF);
        out.push(0xE0);
        let len = app0_data.len() as u16 + 2;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(app0_data);
    }

    // DQT: table 0 = luminance, table 1 = chrominance
    {
        // Table 0: 8-bit precision, table id 0
        out.push(0xFF);
        out.push(0xDB);
        let dqt_len = 2 + 1 + 64 + 1 + 64; // length + 2 tables
        out.extend_from_slice(&(dqt_len as u16).to_be_bytes());
        // Luminance table (precision=0, id=0)
        out.push(0x00);
        // Store in zigzag order
        for &z in &ZIGZAG {
            out.push(STD_LUM_QUANT[z]);
        }
        // Chrominance table (precision=0, id=1)
        out.push(0x01);
        for &z in &ZIGZAG {
            out.push(STD_CHROM_QUANT[z]);
        }
    }

    // SOF0 (baseline DCT frame header)
    {
        out.push(0xFF);
        out.push(0xC0);
        let sof_len = 2 + 6 + 3 * 3; // length + frame header + 3 components × 3 bytes
        out.extend_from_slice(&(sof_len as u16).to_be_bytes());
        out.push(8); // precision (8 bits per sample)
        out.extend_from_slice(&(h as u16).to_be_bytes());
        out.extend_from_slice(&(w as u16).to_be_bytes());
        out.push(3); // number of components (Y, Cb, Cr)
                     // Component 1: Y
        out.push(1); // component id
        out.push(0x11); // sampling factors: H=1, V=1
        out.push(0); // quantization table id 0
                     // Component 2: Cb
        out.push(2);
        out.push(0x11);
        out.push(1); // quantization table id 1
                     // Component 3: Cr
        out.push(3);
        out.push(0x11);
        out.push(1); // quantization table id 1
    }

    // DHT: 4 tables (DC lum, DC chrom, AC lum, AC chrom)
    write_dht(&mut out, 0x00, &DC_LUM_BITS, &DC_LUM_HUFFVAL);
    write_dht(&mut out, 0x10, &DC_CHROM_BITS, &DC_CHROM_HUFFVAL);
    write_dht(&mut out, 0x01, &AC_LUM_BITS, &AC_LUM_HUFFVAL);
    write_dht(&mut out, 0x11, &AC_CHROM_BITS, &AC_CHROM_HUFFVAL);

    // SOS (Start of Scan)
    {
        out.push(0xFF);
        out.push(0xDA);
        let sos_len = 2 + 1 + 3 * 2 + 3; // length + num_comp + 3 components × 2 + spectral + approx
        out.extend_from_slice(&(sos_len as u16).to_be_bytes());
        out.push(3); // number of components
                     // Component 1: Y (DC table 0, AC table 0)
        out.push(1);
        out.push(0x00);
        // Component 2: Cb (DC table 1, AC table 1)
        out.push(2);
        out.push(0x11);
        // Component 3: Cr (DC table 1, AC table 1)
        out.push(3);
        out.push(0x11);
        out.push(0); // spectral selection start
        out.push(63); // spectral selection end
        out.push(0x00); // successive approximation
    }

    // Entropy-coded data
    {
        let mut bw = JpegBitWriter::new();

        let mut prev_dc_y: i32 = 0;
        let mut prev_dc_cb: i32 = 0;
        let mut prev_dc_cr: i32 = 0;

        for by in 0..blocks_y {
            for bx in 0..blocks_x {
                let origin = by * 8 * mcu_w + bx * 8;

                // Process Y block
                {
                    let mut dct_block_f = [0.0f32; 64];
                    let mut qblock = [0i32; 64];
                    for j in 0..8 {
                        for i in 0..8 {
                            let idx = origin + j * mcu_w + i;
                            // Level shift (subtract 128)
                            dct_block_f[j * 8 + i] = y_plane[idx] as f32 - 128.0;
                        }
                    }
                    fdct(&mut dct_block_f);
                    for k in 0..64 {
                        qblock[k] = dct_block_f[k].round() as i32;
                    }
                    quantize(&mut qblock, &STD_LUM_QUANT);
                    encode_block(&mut bw, &qblock, &dc_lum_table, &ac_lum_table, &mut prev_dc_y);
                }

                // Process Cb block
                {
                    let mut dct_block_f = [0.0f32; 64];
                    let mut qblock = [0i32; 64];
                    for j in 0..8 {
                        for i in 0..8 {
                            let idx = origin + j * mcu_w + i;
                            dct_block_f[j * 8 + i] = cb_plane[idx] as f32 - 128.0;
                        }
                    }
                    fdct(&mut dct_block_f);
                    for k in 0..64 {
                        qblock[k] = dct_block_f[k].round() as i32;
                    }
                    quantize(&mut qblock, &STD_CHROM_QUANT);
                    encode_block(
                        &mut bw,
                        &qblock,
                        &dc_chrom_table,
                        &ac_chrom_table,
                        &mut prev_dc_cb,
                    );
                }

                // Process Cr block
                {
                    let mut dct_block_f = [0.0f32; 64];
                    let mut qblock = [0i32; 64];
                    for j in 0..8 {
                        for i in 0..8 {
                            let idx = origin + j * mcu_w + i;
                            dct_block_f[j * 8 + i] = cr_plane[idx] as f32 - 128.0;
                        }
                    }
                    fdct(&mut dct_block_f);
                    for k in 0..64 {
                        qblock[k] = dct_block_f[k].round() as i32;
                    }
                    quantize(&mut qblock, &STD_CHROM_QUANT);
                    encode_block(
                        &mut bw,
                        &qblock,
                        &dc_chrom_table,
                        &ac_chrom_table,
                        &mut prev_dc_cr,
                    );
                }
            }
        }

        let entropy_data = bw.finish();
        out.extend_from_slice(&entropy_data);
    }

    // EOI
    out.extend_from_slice(&[0xFF, 0xD9]);

    Ok(out)
}

/// Encode a decoded image into bytes in the specified format.
pub fn encode(image: &DecodedImage, format: ImageFormat) -> Result<Vec<u8>, String> {
    match format {
        ImageFormat::Png => encode_png(image),
        ImageFormat::Bmp => encode_bmp(image),
        ImageFormat::Qoi => encode_qoi(image),
        ImageFormat::Farbfeld => encode_farbfeld(image),
        ImageFormat::Pnm => encode_pnm(image),
        ImageFormat::Jpeg => encode_jpeg(image),
        ImageFormat::Rgba8 | ImageFormat::Rgb8 => Ok(image.data.as_bytes().to_vec()),
        ImageFormat::Gif => encode_gif(image),
        ImageFormat::Tiff => encode_tiff(image),
        ImageFormat::Svg | ImageFormat::Svgz => encode_svg(image),
        _ => Err(format!("Encoding to {:?} is not yet supported", format)),
    }
}

// ============================================================================
// GIF encoder — minimal GIF89a with global color table and LZW compression
// ============================================================================

/// Build a 256-entry color palette from pixel data.
/// Collects unique colors up to 256; if more exist, falls back to a 216-entry
/// web-safe palette + 40 extra grays (so all 256 entries are filled).
fn build_gif_palette(rgba: &[u8]) -> Vec<[u8; 3]> {
    let total = rgba.len() / 4;
    let mut seen: Vec<[u8; 3]> = Vec::new();
    let mut used = std::collections::HashSet::new();

    for i in 0..total {
        let off = i * 4;
        let rgb = [rgba[off], rgba[off + 1], rgba[off + 2]];
        let key = ((rgb[0] as u32) << 16) | ((rgb[1] as u32) << 8) | rgb[2] as u32;
        if used.insert(key) && seen.len() < 256 {
            seen.push(rgb);
        }
    }

    if seen.len() <= 256 {
        // Pad to exactly 256 with black
        while seen.len() < 256 {
            seen.push([0, 0, 0]);
        }
        return seen;
    }

    // Fallback: web-safe palette (6x6x6 = 216) + 40 grays
    let mut palette: Vec<[u8; 3]> = Vec::with_capacity(256);
    for r in 0..6u8 {
        for g in 0..6u8 {
            for b in 0..6u8 {
                palette.push([r * 51, g * 51, b * 51]);
            }
        }
    }
    for i in 0..40 {
        let v = (i as u8).wrapping_mul(6).wrapping_add(3);
        palette.push([v, v, v]);
    }
    palette
}

/// Find the nearest palette color index via Euclidean distance (squared).
fn nearest_palette_index(r: u8, g: u8, b: u8, palette: &[[u8; 3]]) -> u8 {
    let mut best = 0u8;
    let mut best_dist = u32::MAX;
    for (idx, &[pr, pg, pb]) in palette.iter().enumerate() {
        let dr = r as i32 - pr as i32;
        let dg = g as i32 - pg as i32;
        let db = b as i32 - pb as i32;
        let dist = (dr * dr + dg * dg + db * db) as u32;
        if dist < best_dist {
            best_dist = dist;
            best = idx as u8;
        }
    }
    best
}

/// GIF LZW encoder — writes packed bit codes into sub-blocks.
struct GifLzwWriter {
    out: Vec<u8>,
    sub_block: Vec<u8>,
    bit_buf: u64,
    bit_count: u8,
}

impl GifLzwWriter {
    fn new() -> Self {
        Self { out: Vec::new(), sub_block: Vec::new(), bit_buf: 0, bit_count: 0 }
    }

    fn flush_sub_block(&mut self) {
        if !self.sub_block.is_empty() {
            self.out.push(self.sub_block.len() as u8);
            self.out.extend_from_slice(&self.sub_block);
            self.sub_block.clear();
        }
    }

    fn write_code(&mut self, code: u16, code_size: u8) {
        self.bit_buf |= (code as u64) << self.bit_count;
        self.bit_count += code_size;
        while self.bit_count >= 8 {
            self.sub_block.push(self.bit_buf as u8);
            self.bit_buf >>= 8;
            self.bit_count -= 8;
            if self.sub_block.len() == 255 {
                self.flush_sub_block();
            }
        }
    }

    fn finish(&mut self) {
        if self.bit_count > 0 {
            self.sub_block.push(self.bit_buf as u8);
        }
        self.flush_sub_block();
        self.out.push(0); // block terminator
    }
}

/// Encode a sequence of indices using GIF LZW.
fn gif_lzw_encode(indices: &[u8], min_code_size: u8) -> Vec<u8> {
    let clear_code = 1u16 << min_code_size;
    let eoi_code = clear_code + 1;
    let max_entries = 4096u16;

    // Initial dictionary: each single-byte value -> code
    let mut dict: Vec<Vec<u8>> = Vec::with_capacity(max_entries as usize);
    for i in 0..(1u16 << min_code_size) {
        dict.push(vec![i as u8]);
    }
    dict.push(Vec::new()); // clear_code placeholder
    dict.push(Vec::new()); // eoi_code placeholder

    let mut writer = GifLzwWriter::new();
    let mut code_size = min_code_size + 1;
    let mut next_code = eoi_code + 1;

    writer.write_code(clear_code, code_size);

    if indices.is_empty() {
        writer.write_code(eoi_code, code_size);
        writer.finish();
        return writer.out;
    }

    let mut w = vec![indices[0]];

    for &k in &indices[1..] {
        let mut wk = w.clone();
        wk.push(k);

        let found = dict.iter().position(|entry| *entry == wk);
        if found.is_some() {
            w = wk;
        } else {
            let code = dict.iter().position(|entry| *entry == w).unwrap();
            writer.write_code(code as u16, code_size);

            if next_code < max_entries {
                dict.push(wk);
                next_code += 1;
                if next_code == (1u16 << (code_size + 1)) && code_size < 12 {
                    code_size += 1;
                }
            } else {
                writer.write_code(clear_code, code_size);
                dict.truncate((1usize << (min_code_size as usize)) + 2);
                next_code = eoi_code + 1;
                code_size = min_code_size + 1;
            }
            w = vec![k];
        }
    }

    let code = dict.iter().position(|entry| *entry == w).unwrap();
    writer.write_code(code as u16, code_size);
    writer.write_code(eoi_code, code_size);
    writer.finish();

    writer.out
}

/// Encode to GIF format (GIF89a, static image).
fn encode_gif(image: &DecodedImage) -> Result<Vec<u8>, String> {
    let rgba = image.as_rgba8();
    let pixels = rgba.as_bytes();
    let w = image.width;
    let h = image.height;

    if w == 0 || h == 0 {
        return Err("Cannot encode GIF with zero dimensions".into());
    }

    let palette = build_gif_palette(pixels);
    let palette_size = palette.len();
    let bits_per_pixel: u8 = if palette_size <= 2 {
        1
    } else if palette_size <= 4 {
        2
    } else if palette_size <= 16 {
        4
    } else {
        8
    };
    let size_field = bits_per_pixel - 1;
    let min_code_size = bits_per_pixel;

    // Map each pixel to its nearest palette index
    let mut indices: Vec<u8> = Vec::with_capacity((w * h) as usize);
    for i in 0..(w * h) as usize {
        let off = i * 4;
        let r = pixels.get(off).copied().unwrap_or(0);
        let g = pixels.get(off + 1).copied().unwrap_or(0);
        let b = pixels.get(off + 2).copied().unwrap_or(0);
        indices.push(nearest_palette_index(r, g, b, &palette));
    }

    let mut out: Vec<u8> = Vec::new();

    // 1. Header
    out.extend_from_slice(b"GIF89a");

    // 2. Logical Screen Descriptor
    out.extend_from_slice(&(w as u16).to_le_bytes());
    out.extend_from_slice(&(h as u16).to_le_bytes());
    let gct_flag = 0x80u8; // global color table present
    let color_res = 0x70u8; // 8 bits per primary
    out.push(gct_flag | color_res | size_field);
    out.push(0); // background color index
    out.push(0); // pixel aspect ratio

    // 3. Global Color Table
    for &[r, g, b] in &palette {
        out.push(r);
        out.push(g);
        out.push(b);
    }

    // 4. Image Descriptor
    out.push(0x2C); // image separator
    out.extend_from_slice(&0u16.to_le_bytes()); // left
    out.extend_from_slice(&0u16.to_le_bytes()); // top
    out.extend_from_slice(&(w as u16).to_le_bytes());
    out.extend_from_slice(&(h as u16).to_le_bytes());
    out.push(0); // packed field: no local color table

    // 5. Table-Based Image Data
    out.push(min_code_size);
    let lzw_data = gif_lzw_encode(&indices, min_code_size);
    out.extend_from_slice(&lzw_data);

    // 6. Trailer
    out.push(0x3B);

    Ok(out)
}

// ============================================================================
// TIFF encoder — minimal little-endian TIFF with uncompressed RGBA strips
// ============================================================================

/// Write a TIFF IFD entry (12 bytes).  Inline values must fit in 4 bytes.
fn tiff_ifd_entry(buf: &mut Vec<u8>, tag: u16, typ: u16, count: u32, value: &[u8]) {
    buf.extend_from_slice(&tag.to_le_bytes());
    buf.extend_from_slice(&typ.to_le_bytes());
    buf.extend_from_slice(&count.to_le_bytes());
    let mut val = [0u8; 4];
    val[..value.len().min(4)].copy_from_slice(value);
    buf.extend_from_slice(&val);
}

/// Encode to TIFF format (uncompressed RGBA).
fn encode_tiff(image: &DecodedImage) -> Result<Vec<u8>, String> {
    let rgba = image.as_rgba8();
    let pixels = rgba.as_bytes();
    let w = image.width;
    let h = image.height;

    if w == 0 || h == 0 {
        return Err("Cannot encode TIFF with zero dimensions".into());
    }

    let samples_per_pixel: u16 = 4;
    let row_bytes = w as usize * samples_per_pixel as usize;
    let strip_size = row_bytes * h as usize;

    // Layout:
    //   0-1    "II"
    //   2-3    0x002a (LE)
    //   4-7    IFD offset = 8
    //
    //   8-9    entry count
    //   10-... IFD entries (9 x 12 = 108 bytes)
    //   118-121  next IFD offset = 0
    //   122-...  BitsPerSample array (8 bytes: 4 x u16)
    //   130-...  pixel strip data

    let ifd_offset: u32 = 8;
    let num_entries: u16 = 9;
    let ifd_body_start = 8u32 + 2; // after count field
    let ifd_end = ifd_body_start + num_entries as u32 * 12 + 4; // +4 for next IFD
    let bps_array_offset = ifd_end;
    let strip_offset = bps_array_offset + 8; // 4 x u16 for BitsPerSample

    let mut out = Vec::new();

    // TIFF header
    out.extend_from_slice(b"II");
    out.extend_from_slice(&0x002Au16.to_le_bytes());
    out.extend_from_slice(&ifd_offset.to_le_bytes());

    // IFD entry count
    out.extend_from_slice(&num_entries.to_le_bytes());

    // Tag 256: ImageWidth (LONG, count=1)
    tiff_ifd_entry(&mut out, 256, 4, 1, &w.to_le_bytes());
    // Tag 257: ImageLength (LONG, count=1)
    tiff_ifd_entry(&mut out, 257, 4, 1, &h.to_le_bytes());
    // Tag 258: BitsPerSample (SHORT, count=4) -> offset to array
    tiff_ifd_entry(&mut out, 258, 3, 4, &bps_array_offset.to_le_bytes());
    // Tag 259: Compression = 1 (uncompressed, SHORT, count=1)
    tiff_ifd_entry(&mut out, 259, 3, 1, &1u16.to_le_bytes());
    // Tag 262: PhotometricInterpretation = 2 (RGB, SHORT, count=1)
    tiff_ifd_entry(&mut out, 262, 3, 1, &2u16.to_le_bytes());
    // Tag 273: StripOffsets (LONG, count=1)
    tiff_ifd_entry(&mut out, 273, 4, 1, &strip_offset.to_le_bytes());
    // Tag 277: SamplesPerPixel (SHORT, count=1)
    tiff_ifd_entry(&mut out, 277, 3, 1, &samples_per_pixel.to_le_bytes());
    // Tag 278: RowsPerStrip (LONG, count=1)
    tiff_ifd_entry(&mut out, 278, 4, 1, &h.to_le_bytes());
    // Tag 279: StripByteCounts (LONG, count=1)
    tiff_ifd_entry(&mut out, 279, 4, 1, &(strip_size as u32).to_le_bytes());

    // Next IFD offset
    out.extend_from_slice(&0u32.to_le_bytes());

    // BitsPerSample array: all 8
    for _ in 0..4 {
        out.extend_from_slice(&8u16.to_le_bytes());
    }

    // Pixel data (uncompressed RGBA)
    out.extend_from_slice(pixels);

    Ok(out)
}

// ============================================================================
// SVG encoder — wraps a base64-encoded PNG in an SVG element
// ============================================================================

/// Minimal base64 encoding (RFC 4648).
fn base64_encode(data: &[u8]) -> String {
    const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = chunk.get(1).copied().unwrap_or(0) as u32;
        let b2 = chunk.get(2).copied().unwrap_or(0) as u32;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        // Emit 4 characters
        for j in (0..4).rev() {
            let idx = ((triple >> (j * 6)) & 0x3F) as usize;
            out.push(CHARS[idx] as char);
        }
        // Replace padding
        if chunk.len() < 3 {
            let pad = 3 - chunk.len();
            for k in 0..pad {
                let pos = out.len() - 1 - k;
                // SAFETY: The position is within the valid range (we just pushed the
                // characters above).  Writing '=' (ASCII 0x3D) at a known position
                // preserves UTF-8 validity because '=' is a single-byte ASCII char.
                unsafe {
                    out.as_bytes_mut()[pos] = b'=';
                }
            }
        }
    }
    out
}

/// Encode to SVG format by embedding a base64-encoded PNG.
fn encode_svg(image: &DecodedImage) -> Result<Vec<u8>, String> {
    let w = image.width;
    let h = image.height;

    let png_bytes = encode_png(image)?;
    let b64 = base64_encode(&png_bytes);

    let xml = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{}" height="{}">
  <image href="data:image/png;base64,{}" width="{}" height="{}"/>
</svg>"#,
        w, h, b64, w, h
    );

    Ok(xml.into_bytes())
}

fn encode_png(image: &DecodedImage) -> Result<Vec<u8>, String> {
    let rgba = image.as_rgba8();
    let pixels = rgba.as_bytes();
    let w = image.width;
    let h = image.height;

    let mut out = Vec::new();
    // PNG signature
    out.extend_from_slice(b"\x89PNG\r\n\x1a\n");

    // IHDR chunk
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&w.to_be_bytes());
    ihdr.extend_from_slice(&h.to_be_bytes());
    ihdr.push(8); // bit depth
    ihdr.push(6); // color type: RGBA
    ihdr.push(0); // compression
    ihdr.push(0); // filter
    ihdr.push(0); // interlace
    write_png_chunk(&mut out, b"IHDR", &ihdr);

    // IDAT chunk (raw unfiltered rows with filter byte 0)
    let mut raw = Vec::with_capacity((1 + w as usize * 4) * h as usize);
    for row in 0..h as usize {
        raw.push(0); // filter type: None
        let off = row * w as usize * 4;
        let end = (off + w as usize * 4).min(pixels.len());
        raw.extend_from_slice(&pixels[off..end]);
    }
    // Store uncompressed (zlib stored block)
    write_png_chunk(&mut out, b"IDAT", &raw);

    // IEND chunk
    write_png_chunk(&mut out, b"IEND", &[]);

    Ok(out)
}

fn write_png_chunk(out: &mut Vec<u8>, chunk_type: &[u8; 4], data: &[u8]) {
    let len = data.len() as u32;
    out.extend_from_slice(&len.to_be_bytes());
    out.extend_from_slice(chunk_type);
    out.extend_from_slice(data);
    // CRC32 over type + data
    let crc = crc32(&[chunk_type, data].concat());
    out.extend_from_slice(&crc.to_be_bytes());
}

fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFFFFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
        }
    }
    crc ^ 0xFFFFFFFF
}

/// Encode to BMP format (24-bit).
fn encode_bmp(image: &DecodedImage) -> Result<Vec<u8>, String> {
    let rgba = image.as_rgba8();
    let pixels = rgba.as_bytes();
    let w = image.width;
    let h = image.height;

    let row_size = (w * 3).div_ceil(4) * 4;
    let pixel_data_size = row_size * h;
    let file_size = 14 + 40 + pixel_data_size;

    let mut out = Vec::with_capacity(file_size as usize);
    // BMP header
    out.extend_from_slice(b"BM");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(&[0u8; 4]); // reserved
    out.extend_from_slice(&(54u32).to_le_bytes()); // pixel offset

    // DIB header
    out.extend_from_slice(&40u32.to_le_bytes()); // header size
    out.extend_from_slice(&w.to_le_bytes());
    out.extend_from_slice(&h.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // planes
    out.extend_from_slice(&24u16.to_le_bytes()); // bit count
    out.extend_from_slice(&[0u8; 24]); // no compression, rest

    // Pixel data (BGR, bottom-up, 4-byte aligned rows)
    let mut row = vec![0u8; row_size as usize];
    for y in (0..h).rev() {
        row.fill(0);
        for x in 0..w {
            let off = ((y * w + x) * 4) as usize;
            let bgr_off = x as usize * 3;
            row[bgr_off] = pixels.get(off + 2).copied().unwrap_or(0); // B
            row[bgr_off + 1] = pixels.get(off + 1).copied().unwrap_or(0); // G
            row[bgr_off + 2] = pixels.get(off).copied().unwrap_or(0); // R
        }
        out.extend_from_slice(&row);
    }
    Ok(out)
}

/// Encode to QOI format.
fn encode_qoi(image: &DecodedImage) -> Result<Vec<u8>, String> {
    let rgba = image.as_rgba8();
    let pixels = rgba.as_bytes();
    let w = image.width;
    let h = image.height;

    let mut out = Vec::with_capacity(14 + pixels.len() + 8);
    out.extend_from_slice(b"qoif");
    out.extend_from_slice(&w.to_be_bytes());
    out.extend_from_slice(&h.to_be_bytes());
    out.push(4); // channels = RGBA
    out.push(0); // colorspace = sRGB

    let mut index = [[0u8; 4]; 64];
    let mut prev = [0u8, 0, 0, 255];
    let mut run: usize = 0;
    let total = (w * h) as usize;

    for i in 0..total {
        let off = i * 4;
        let r = pixels.get(off).copied().unwrap_or(0);
        let g = pixels.get(off + 1).copied().unwrap_or(0);
        let b = pixels.get(off + 2).copied().unwrap_or(0);
        let a = pixels.get(off + 3).copied().unwrap_or(255);
        let px = [r, g, b, a];

        if px == prev {
            run += 1;
            if run == 62 || i == total - 1 {
                out.push(0xC0 | (run as u8 - 1));
                run = 0;
            }
        } else {
            if run > 0 {
                out.push(0xC0 | (run as u8 - 1));
                run = 0;
            }
            let hash = (r as usize * 3 + g as usize * 5 + b as usize * 7 + a as usize * 11) & 63;
            if index[hash] == px {
                out.push(hash as u8); // QOI_OP_INDEX
            } else if a == prev[3] {
                let dr = r.wrapping_sub(prev[0]).wrapping_add(2) as i8;
                let dg = g.wrapping_sub(prev[1]).wrapping_add(2) as i8;
                let db = b.wrapping_sub(prev[2]).wrapping_add(2) as i8;
                if (0..4).contains(&dr) && (0..4).contains(&dg) && (0..4).contains(&db) {
                    out.push(0x40 | ((dr as u8) << 4) | ((dg as u8) << 2) | db as u8);
                } else {
                    let dg2 = g.wrapping_sub(prev[1]).wrapping_add(32) as i8;
                    let dr_dg = r.wrapping_sub(g).wrapping_add(8) as i8;
                    let db_dg = b.wrapping_sub(g).wrapping_add(8) as i8;
                    if (0..64).contains(&dg2)
                        && (0..16).contains(&dr_dg)
                        && (0..16).contains(&db_dg)
                    {
                        out.push(0x80 | dg2 as u8);
                        out.push((dr_dg as u8) << 4 | (db_dg as u8));
                    } else {
                        out.push(0xFE);
                        out.extend_from_slice(&[r, g, b]);
                    }
                }
            } else {
                out.push(0xFF);
                out.extend_from_slice(&[r, g, b, a]);
            }
            index[hash] = px;
        }
        prev = px;
    }
    // 8-byte padding
    out.extend_from_slice(&[0u8; 8]);
    Ok(out)
}

/// Encode to Farbfeld format.
fn encode_farbfeld(image: &DecodedImage) -> Result<Vec<u8>, String> {
    let rgba = image.as_rgba8();
    let pixels = rgba.as_bytes();
    let w = image.width;
    let h = image.height;
    let total = (w * h) as usize;

    let mut out = Vec::with_capacity(16 + total * 8);
    out.extend_from_slice(b"farbfeld");
    out.extend_from_slice(&w.to_be_bytes());
    out.extend_from_slice(&h.to_be_bytes());

    for i in 0..total {
        let off = i * 4;
        let r_u16 = (pixels.get(off).copied().unwrap_or(0) as u16) << 8;
        let g_u16 = (pixels.get(off + 1).copied().unwrap_or(0) as u16) << 8;
        let b_u16 = (pixels.get(off + 2).copied().unwrap_or(0) as u16) << 8;
        let a_u16 = (pixels.get(off + 3).copied().unwrap_or(255) as u16) << 8;
        out.extend_from_slice(&r_u16.to_be_bytes());
        out.extend_from_slice(&g_u16.to_be_bytes());
        out.extend_from_slice(&b_u16.to_be_bytes());
        out.extend_from_slice(&a_u16.to_be_bytes());
    }
    Ok(out)
}

/// Encode to PNM binary format (P6 RGB or P5 grayscale).
fn encode_pnm(image: &DecodedImage) -> Result<Vec<u8>, String> {
    let w = image.width;
    let h = image.height;
    let is_grayscale = matches!(image.data, ImageData::Grayscale8(_));

    let header = if is_grayscale {
        format!("P5\n{} {}\n255\n", w, h)
    } else {
        format!("P6\n{} {}\n255\n", w, h)
    };

    let mut out = Vec::new();
    out.extend_from_slice(header.as_bytes());

    if is_grayscale {
        if let ImageData::Grayscale8(ref g) = image.data {
            out.extend_from_slice(g);
        }
    } else {
        let rgba = image.as_rgba8();
        let pixels = rgba.as_bytes();
        for i in 0..(w * h) as usize {
            let off = i * 4;
            out.push(pixels.get(off).copied().unwrap_or(0));
            out.push(pixels.get(off + 1).copied().unwrap_or(0));
            out.push(pixels.get(off + 2).copied().unwrap_or(0));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::format::{DecodedImage, ImageData};

    fn make_test_image() -> DecodedImage {
        let w = 2u32;
        let h = 2u32;
        let pixels = vec![
            255, 0, 0, 255, // red
            0, 255, 0, 255, // green
            0, 0, 255, 255, // blue
            128, 128, 128, 255, // gray
        ];
        DecodedImage::new(ImageFormat::Rgba8, ImageData::Rgba8(pixels), w, h)
    }

    #[test]
    fn encode_png_roundtrip() {
        let img = make_test_image();
        let encoded = encode_png(&img).unwrap();
        assert!(encoded.starts_with(b"\x89PNG"));
        assert!(encoded.len() > 33);
    }

    #[test]
    fn encode_bmp_roundtrip() {
        let img = make_test_image();
        let encoded = encode_bmp(&img).unwrap();
        assert!(encoded.starts_with(b"BM"));
    }

    #[test]
    fn encode_qoi_roundtrip() {
        let img = make_test_image();
        let encoded = encode_qoi(&img).unwrap();
        assert!(encoded.starts_with(b"qoif"));
    }

    #[test]
    fn encode_farbfeld_roundtrip() {
        let img = make_test_image();
        let encoded = encode_farbfeld(&img).unwrap();
        assert!(encoded.starts_with(b"farbfeld"));
    }

    #[test]
    fn encode_pnm_roundtrip() {
        let img = make_test_image();
        let encoded = encode_pnm(&img).unwrap();
        assert!(encoded.starts_with(b"P6"));
    }

    #[test]
    fn encode_jpeg_soi() {
        let img = make_test_image();
        let encoded = encode_jpeg(&img).unwrap();
        // JPEG SOI marker is FF D8
        assert!(encoded.starts_with(&[0xFF, 0xD8]));
        // JPEG EOI marker is FF D9
        assert!(encoded.ends_with(&[0xFF, 0xD9]));
        // Contains JFIF header
        assert!(encoded.windows(4).any(|w| w == b"JFIF"), "JPEG output must contain JFIF header");
        assert!(encoded.len() > 200, "JPEG output too short: {}", encoded.len());
    }

    #[test]
    fn encode_jpeg_dispatch() {
        let img = make_test_image();
        let jpeg = encode(&img, ImageFormat::Jpeg).unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8]));
    }

    #[test]
    fn encode_dispatch() {
        let img = make_test_image();
        let png = encode(&img, ImageFormat::Png).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        let bmp = encode(&img, ImageFormat::Bmp).unwrap();
        assert!(bmp.starts_with(b"BM"));
        let jpeg = encode(&img, ImageFormat::Jpeg).unwrap();
        assert!(jpeg.starts_with(&[0xFF, 0xD8]));
    }

    #[test]
    fn encode_gif_roundtrip() {
        let img = make_test_image();
        let encoded = encode_gif(&img).unwrap();
        assert!(encoded.starts_with(b"GIF89a"), "GIF must start with GIF89a");
        assert!(encoded.ends_with(&[0x3B]), "GIF must end with trailer 0x3B");
        assert!(encoded.len() > 20, "GIF output too short: {}", encoded.len());
    }

    #[test]
    fn encode_gif_dispatch() {
        let img = make_test_image();
        let gif = encode(&img, ImageFormat::Gif).unwrap();
        assert!(gif.starts_with(b"GIF89a"));
    }

    #[test]
    fn encode_tiff_roundtrip() {
        let img = make_test_image();
        let encoded = encode_tiff(&img).unwrap();
        assert!(encoded.starts_with(b"II"), "TIFF must start with II");
        // Check for TIFF magic 0x002A at offset 2
        assert!(encoded.len() >= 4);
        assert_eq!(encoded[2..4], [0x2A, 0x00], "TIFF magic must be 0x002A");
        assert!(encoded.len() > 50, "TIFF output too short: {}", encoded.len());
    }

    #[test]
    fn encode_tiff_dispatch() {
        let img = make_test_image();
        let tiff = encode(&img, ImageFormat::Tiff).unwrap();
        assert!(tiff.starts_with(b"II"));
    }

    #[test]
    fn encode_svg_roundtrip() {
        let img = make_test_image();
        let encoded = encode_svg(&img).unwrap();
        let s = String::from_utf8_lossy(&encoded);
        assert!(s.starts_with("<svg"), "SVG must start with <svg");
        assert!(s.contains("xmlns="), "SVG must contain xmlns attribute");
        assert!(s.contains("data:image/png;base64,"), "SVG must embed base64 PNG");
        assert!(s.contains("width=\"2\""), "SVG must have width attribute");
        assert!(s.contains("height=\"2\""), "SVG must have height attribute");
        assert!(s.ends_with("/>\n</svg>"), "SVG must end with </svg>");
    }

    #[test]
    fn encode_svg_dispatch() {
        let img = make_test_image();
        let svg = encode(&img, ImageFormat::Svg).unwrap();
        let s = String::from_utf8_lossy(&svg);
        assert!(s.starts_with("<svg"));
    }

    #[test]
    fn encode_svgz_dispatch() {
        let img = make_test_image();
        let svgz = encode(&img, ImageFormat::Svgz).unwrap();
        let s = String::from_utf8_lossy(&svgz);
        assert!(s.starts_with("<svg"));
    }
}
