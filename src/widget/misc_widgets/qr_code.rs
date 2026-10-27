// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! QRCode widget — renders a real byte-mode QR Code symbol.
//!
//! The widget contains a dependency-free QR encoder (ISO/IEC 18004) for byte mode. It supports
//! versions 1 through 40 at all four error-correction levels, writes the mode and
//! character-count indicators, appends Reed–Solomon codewords, chooses the mask with the lowest
//! penalty score, and emits the function patterns and format/version information. The resulting
//! module grid is a real QR symbol: a conforming scanner can decode it when the drawing is large
//! enough and the quiet zone is wide enough.
//!
//! Supported envelope:
//!
//! * **Byte, numeric and alphanumeric modes.** The encoder plans runs of digits and
//!   alphanumeric characters, and falls back to UTF-8 byte mode for everything else.
//! * **Versions 1–40, all four EC levels.** The version is chosen automatically, smallest first.
//!   A payload that does not fit version 40 at the selected level is reported through the
//!   widget's `encode_error` field, and `draw` renders an explicit error placeholder instead of a
//!   QR-like matrix.
//! * **Real format information.** The 15-bit format field is BCH-encoded with the standard
//!   generator and `0x5412` mask; versions 7 and above also carry the 18-bit version information.
//!
//! The character-count indicator is 8 bits wide for versions 1–9 and 16 bits wide for byte mode
//! at version 10 and above, as the specification requires at that boundary.

use std::fmt;

use crate::core::{Color, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::widget::capability::coercion::{expect_string, expect_u32_saturating, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::{BaseWidget, Draw, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Side of a finder pattern (and of its separator ring) in modules.
const FINDER_SIZE: u32 = 7;

/// The four error-correction levels the QR specification defines.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QrEcLevel {
    /// Recovers about 7% of codewords.
    L,
    /// Recovers about 15% of codewords.
    M,
    /// Recovers about 25% of codewords.
    Q,
    /// Recovers about 30% of codewords.
    H,
}

impl QrEcLevel {
    /// The two-bit value used by the format information.
    fn format_bits(self) -> u8 {
        match self {
            Self::L => 0b01,
            Self::M => 0b00,
            Self::Q => 0b11,
            Self::H => 0b10,
        }
    }

    /// The token this level publishes through the property contract.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::L => "L",
            Self::M => "M",
            Self::Q => "Q",
            Self::H => "H",
        }
    }

    /// Parses [`Self::as_str`].
    pub fn from_token(token: &str) -> Option<Self> {
        match token.trim().to_ascii_uppercase().as_str() {
            "L" => Some(Self::L),
            "M" => Some(Self::M),
            "Q" => Some(Self::Q),
            "H" => Some(Self::H),
            _ => None,
        }
    }
}

/// An encoding failure. The payload is a human-readable reason.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrEncodeError(pub String);

impl fmt::Display for QrEncodeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A row-major QR module grid. `true` means a dark module.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QrMatrix {
    /// Number of modules on each side (`17 + 4 * version`).
    pub size: u32,
    /// `size * size` modules in row-major order.
    pub modules: Vec<bool>,
}

impl QrMatrix {
    fn blank(size: u32) -> Self {
        Self { size, modules: vec![false; (size * size) as usize] }
    }

    fn get(&self, row: u32, col: u32) -> bool {
        self.modules[(row * self.size + col) as usize]
    }

    fn set(&mut self, row: u32, col: u32, value: bool) {
        self.modules[(row * self.size + col) as usize] = value;
    }
}

/// The module side length for a QR version.
fn matrix_size(version: u8) -> u32 {
    17 + 4 * u32::from(version)
}

/// The QR version encoded by a module side length.
fn matrix_version(size: u32) -> u8 {
    ((size - 17) / 4) as u8
}

/// The width of the byte-mode character-count indicator.
fn count_indicator_bits(version: u8) -> usize {
    if version <= 9 {
        8
    } else {
        16
    }
}

/// Returns the top-left module of the finder block a cell belongs to, together with
/// whether the corner-local coordinates must be mirrored on each axis.
///
/// Each finder pattern owns an 8×8 block: the 7×7 finder itself plus a one-module light
/// separator on its inner edges. The three blocks are the top-left corner, the top-right
/// corner (growing leftward) and the bottom-left corner (growing upward). A cell outside
/// every block is data and returns `None`.
fn finder_corner(size: u32, row: u32, col: u32) -> Option<(u32, u32, bool, bool)> {
    let far = size - FINDER_SIZE - 1;
    let outer = FINDER_SIZE + 1;
    if row < outer && col < outer {
        Some((0, 0, false, false))
    } else if row < outer && col >= far {
        Some((0, far, false, true))
    } else if row >= far && col < outer {
        Some((far, 0, true, false))
    } else {
        None
    }
}

/// Data-codeword and Reed–Solomon layout for one (version, EC level) pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BlockSpec {
    data_codewords: usize,
    ec_per_block: usize,
    num_blocks: usize,
    short_data_codewords: usize,
    short_blocks: usize,
}

/// Returns the block layout for a supported (version, EC level) pair.
///
/// The first `short_blocks` blocks hold `short_data_codewords` data codewords; the remaining
/// blocks hold one more. Block layouts that are not listed return `None`, so callers must turn
/// that into an explicit error rather than guessing a capacity.
fn block_spec(version: u8, ec: QrEcLevel) -> Option<BlockSpec> {
    let (data_codewords, ec_per_block, num_blocks, short_data_codewords, short_blocks) =
        match (version, ec) {
            (1, QrEcLevel::L) => (19, 7, 1, 19, 1),
            (1, QrEcLevel::M) => (16, 10, 1, 16, 1),
            (1, QrEcLevel::Q) => (13, 13, 1, 13, 1),
            (1, QrEcLevel::H) => (9, 17, 1, 9, 1),
            (2, QrEcLevel::L) => (34, 10, 1, 34, 1),
            (2, QrEcLevel::M) => (28, 16, 1, 28, 1),
            (2, QrEcLevel::Q) => (22, 22, 1, 22, 1),
            (2, QrEcLevel::H) => (16, 28, 1, 16, 1),
            (3, QrEcLevel::L) => (55, 15, 1, 55, 1),
            (3, QrEcLevel::M) => (44, 26, 1, 44, 1),
            (3, QrEcLevel::Q) => (34, 18, 2, 17, 2),
            (3, QrEcLevel::H) => (26, 22, 2, 13, 2),
            (4, QrEcLevel::L) => (80, 20, 1, 80, 1),
            (4, QrEcLevel::M) => (64, 18, 2, 32, 2),
            (4, QrEcLevel::Q) => (48, 26, 2, 24, 2),
            (4, QrEcLevel::H) => (36, 16, 4, 9, 4),
            (5, QrEcLevel::L) => (108, 26, 1, 108, 1),
            (5, QrEcLevel::M) => (86, 24, 2, 43, 2),
            (5, QrEcLevel::Q) => (62, 18, 4, 15, 2),
            (5, QrEcLevel::H) => (46, 22, 4, 11, 2),
            (6, QrEcLevel::L) => (136, 18, 2, 68, 2),
            (6, QrEcLevel::M) => (108, 16, 4, 27, 4),
            (6, QrEcLevel::Q) => (76, 24, 4, 19, 4),
            (6, QrEcLevel::H) => (60, 28, 4, 15, 4),
            (7, QrEcLevel::L) => (156, 20, 2, 78, 2),
            (7, QrEcLevel::M) => (124, 18, 4, 31, 4),
            (7, QrEcLevel::Q) => (88, 18, 6, 14, 2),
            (7, QrEcLevel::H) => (66, 26, 5, 13, 4),
            (8, QrEcLevel::L) => (194, 24, 2, 97, 2),
            (8, QrEcLevel::M) => (154, 22, 4, 38, 2),
            (8, QrEcLevel::Q) => (110, 22, 6, 18, 4),
            (8, QrEcLevel::H) => (86, 26, 6, 14, 4),
            (9, QrEcLevel::L) => (232, 30, 2, 116, 2),
            (9, QrEcLevel::M) => (182, 22, 5, 36, 3),
            (9, QrEcLevel::Q) => (132, 20, 8, 16, 4),
            (9, QrEcLevel::H) => (100, 24, 8, 12, 4),
            (10, QrEcLevel::L) => (274, 18, 4, 68, 2),
            (10, QrEcLevel::M) => (216, 26, 5, 43, 4),
            (10, QrEcLevel::Q) => (154, 24, 8, 19, 6),
            (10, QrEcLevel::H) => (122, 28, 8, 15, 6),
            _ => return spec_from_tables(version, ec),
        };
    Some(BlockSpec { data_codewords, ec_per_block, num_blocks, short_data_codewords, short_blocks })
}

/// Reed–Solomon codewords per block, `[L, M, Q, H]`, indexed by version (0 is unused).
///
/// These are the ISO/IEC 18004 tables; versions 1–10 are kept in [`block_spec`] directly, and
/// `spec_from_tables` reproduces them exactly (pinned by a unit test).
const ECC_CODEWORDS_PER_BLOCK: [[u8; 41]; 4] = [
    [
        0, 7, 10, 15, 20, 26, 18, 20, 24, 30, 18, 20, 24, 26, 30, 22, 24, 28, 30, 28, 28, 28, 28,
        30, 30, 26, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
    [
        0, 10, 16, 26, 18, 24, 16, 18, 22, 22, 26, 30, 22, 22, 24, 24, 28, 28, 26, 26, 26, 26, 28,
        28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28, 28,
    ],
    [
        0, 13, 22, 18, 26, 18, 24, 18, 22, 20, 24, 28, 26, 24, 20, 30, 24, 28, 28, 26, 30, 28, 30,
        30, 30, 30, 28, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
    [
        0, 17, 28, 22, 16, 22, 28, 26, 26, 24, 28, 24, 28, 22, 24, 24, 30, 28, 28, 26, 28, 30, 24,
        30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30, 30,
    ],
];

/// Reed–Solomon block counts, `[L, M, Q, H]`, indexed by version (0 is unused).
const NUM_ERROR_CORRECTION_BLOCKS: [[u8; 41]; 4] = [
    [
        0, 1, 1, 1, 1, 1, 2, 2, 2, 2, 4, 4, 4, 4, 4, 6, 6, 6, 6, 7, 8, 8, 9, 9, 10, 12, 12, 12, 13,
        14, 15, 16, 17, 18, 19, 19, 20, 21, 22, 24, 25,
    ],
    [
        0, 1, 1, 1, 2, 2, 4, 4, 4, 5, 5, 5, 8, 9, 9, 10, 10, 11, 13, 14, 16, 17, 17, 18, 20, 21,
        23, 25, 26, 28, 29, 31, 33, 35, 37, 38, 40, 43, 45, 47, 49,
    ],
    [
        0, 1, 1, 2, 2, 4, 4, 6, 6, 8, 8, 8, 10, 12, 16, 12, 17, 16, 18, 21, 20, 23, 23, 25, 27, 29,
        34, 34, 35, 38, 40, 43, 45, 48, 51, 53, 56, 59, 62, 65, 68,
    ],
    [
        0, 1, 1, 2, 4, 4, 4, 5, 6, 8, 8, 11, 11, 16, 16, 18, 16, 19, 21, 25, 25, 25, 34, 30, 32,
        35, 37, 40, 42, 45, 48, 51, 54, 57, 60, 63, 66, 70, 74, 77, 81,
    ],
];

/// The total number of codewords (data plus EC) a version carries.
///
/// The function patterns are subtracted from the raw module count rather than tabulated, so the
/// number cannot drift from the matrix the encoder actually builds.
fn total_codewords(version: u8) -> usize {
    let version = usize::from(version);
    let mut modules = (16 * version + 128) * version + 64;
    if version >= 2 {
        let alignment = version / 7 + 2;
        modules -= (25 * alignment - 10) * alignment - 55;
        if version >= 7 {
            modules -= 36;
        }
    }
    modules / 8
}

/// Derives a block layout from the standard tables for versions 11–40.
///
/// The data capacity is the version's total minus the parity, and the blocks differ by at most
/// one codeword, which is exactly the [`BlockSpec`] shape.
fn spec_from_tables(version: u8, ec: QrEcLevel) -> Option<BlockSpec> {
    if version == 0 || version > 40 {
        return None;
    }
    let index = usize::from(version);
    let row = match ec {
        QrEcLevel::L => 0,
        QrEcLevel::M => 1,
        QrEcLevel::Q => 2,
        QrEcLevel::H => 3,
    };
    let ec_per_block = usize::from(ECC_CODEWORDS_PER_BLOCK[row][index]);
    let num_blocks = usize::from(NUM_ERROR_CORRECTION_BLOCKS[row][index]);
    if ec_per_block == 0 || num_blocks == 0 {
        return None;
    }
    let data_codewords = total_codewords(version).checked_sub(ec_per_block * num_blocks)?;
    let short_data_codewords = data_codewords / num_blocks;
    let short_blocks = num_blocks - data_codewords % num_blocks;
    Some(BlockSpec { data_codewords, ec_per_block, num_blocks, short_data_codewords, short_blocks })
}

/// Places `count` bits of `value`, most significant bit first, into a bit stream.
fn push_bits(out: &mut Vec<bool>, value: u32, count: usize) {
    for shift in (0..count).rev() {
        out.push((value >> shift) & 1 != 0);
    }
}

/// Builds the data codeword stream: mode, count, payload, terminator and padding.
fn build_codewords(data: &[u8], version: u8, spec: BlockSpec) -> Result<Vec<u8>, QrEncodeError> {
    let capacity_bits = spec.data_codewords * 8;
    let mut bits = Vec::with_capacity(capacity_bits);
    push_bits(&mut bits, 0b0100, 4);
    push_bits(&mut bits, data.len() as u32, count_indicator_bits(version));
    for &byte in data {
        push_bits(&mut bits, u32::from(byte), 8);
    }
    assemble_codewords(bits, spec)
}

/// Terminates, pads, blocks, Reed–Solomon-encodes and interleaves a data bit stream.
///
/// Shared by the byte-mode and the auto-segmented encoders so the two cannot disagree about the
/// padding or the block layout.
fn assemble_codewords(mut bits: Vec<bool>, spec: BlockSpec) -> Result<Vec<u8>, QrEncodeError> {
    let capacity_bits = spec.data_codewords * 8;
    if bits.len() > capacity_bits {
        return Err(QrEncodeError(format!(
            "bit stream is {} bits but the selected QR version only holds {}",
            bits.len(),
            capacity_bits
        )));
    }

    // Terminator: up to four zero bits, then zero-fill to a codeword boundary.
    let terminator = (capacity_bits - bits.len()).min(4);
    bits.resize(bits.len() + terminator, false);
    if !bits.len().is_multiple_of(8) {
        bits.resize(bits.len().div_ceil(8) * 8, false);
    }

    let mut data_codewords = Vec::with_capacity(spec.data_codewords);
    for chunk in bits.chunks(8) {
        let mut byte = 0u8;
        for &bit in chunk {
            byte = (byte << 1) | u8::from(bit);
        }
        data_codewords.push(byte);
    }

    // Pad codewords alternate 0xEC / 0x11 until the data capacity is full.
    let mut pad_with_ec = true;
    while data_codewords.len() < spec.data_codewords {
        data_codewords.push(if pad_with_ec { 0xEC } else { 0x11 });
        pad_with_ec = !pad_with_ec;
    }

    // Split the data into the block layout for this version/EC level.
    let mut blocks: Vec<Vec<u8>> = Vec::with_capacity(spec.num_blocks);
    let mut offset = 0usize;
    for block_index in 0..spec.num_blocks {
        let block_len =
            spec.short_data_codewords + if block_index < spec.short_blocks { 0 } else { 1 };
        let end = offset + block_len;
        if end > data_codewords.len() {
            return Err(QrEncodeError(
                "internal error: QR block layout exceeds the data codeword stream".to_string(),
            ));
        }
        blocks.push(data_codewords[offset..end].to_vec());
        offset = end;
    }
    if offset != spec.data_codewords {
        return Err(QrEncodeError(
            "internal error: QR block layout does not consume every data codeword".to_string(),
        ));
    }

    // Reed–Solomon parity for each block.
    let mut ecc_blocks: Vec<Vec<u8>> = Vec::with_capacity(spec.num_blocks);
    for block in &blocks {
        let appended = reed_solomon_append(block, spec.ec_per_block);
        if appended.len() != block.len() + spec.ec_per_block {
            return Err(QrEncodeError(
                "internal error: Reed–Solomon block has an unexpected length".to_string(),
            ));
        }
        ecc_blocks.push(appended[block.len()..].to_vec());
    }

    // Interleave data codewords block-by-block, then parity codewords block-by-block.
    let mut codewords =
        Vec::with_capacity(spec.data_codewords + spec.num_blocks * spec.ec_per_block);
    let max_data_len = blocks.iter().map(Vec::len).max().unwrap_or(0);
    for index in 0..max_data_len {
        for block in &blocks {
            if let Some(&byte) = block.get(index) {
                codewords.push(byte);
            }
        }
    }
    for index in 0..spec.ec_per_block {
        for block in &ecc_blocks {
            if let Some(&byte) = block.get(index) {
                codewords.push(byte);
            }
        }
    }

    if codewords.len() != spec.data_codewords + spec.num_blocks * spec.ec_per_block {
        return Err(QrEncodeError(
            "internal error: interleaved codeword count does not match the block layout"
                .to_string(),
        ));
    }
    Ok(codewords)
}

/// Chooses the smallest version whose data capacity holds `data_len` payload bytes.
fn choose_version_in(
    data_len: usize,
    ec: QrEcLevel,
    min_version: u8,
    max_version: u8,
) -> Result<u8, QrEncodeError> {
    let min_version = min_version.clamp(1, 40);
    let max_version = max_version.clamp(min_version, 40);
    for version in min_version..=max_version {
        // The 8-bit count indicator of versions 1–9 cannot describe more than 255 bytes.
        if version <= 9 && data_len > 255 {
            continue;
        }
        let spec = match block_spec(version, ec) {
            Some(spec) => spec,
            None => continue,
        };
        let needed_bits = data_len
            .checked_mul(8)
            .and_then(|bits| bits.checked_add(4 + count_indicator_bits(version)))
            .ok_or_else(|| {
                QrEncodeError("data length overflows the encoder's bit budget".to_string())
            })?;
        if needed_bits.div_ceil(8) <= spec.data_codewords {
            return Ok(version);
        }
    }
    Err(QrEncodeError(format!(
        "byte-mode payload of {data_len} bytes does not fit any supported QR version ({}..={}) at \
         EC level {ec:?}",
        min_version, max_version
    )))
}

/// Builds the mask-independent part of the symbol: function patterns plus the data bits.
fn build_base_matrix(version: u8) -> (QrMatrix, Vec<bool>) {
    let size = matrix_size(version);
    let mut matrix = QrMatrix::blank(size);
    let function = function_modules(size, version);

    draw_finders(&mut matrix);
    draw_timing(&mut matrix);
    draw_alignment_patterns(&mut matrix, version);
    // The dark module sits below the top-left format information.
    matrix.set(size - 8, 8, true);
    if version >= 7 {
        draw_version_info(&mut matrix, version);
    }

    (matrix, function)
}

/// Marks every module that belongs to a function pattern or a reserved information area.
fn function_modules(size: u32, version: u8) -> Vec<bool> {
    let mut function = vec![false; (size * size) as usize];
    let mark = |function: &mut [bool], row: u32, col: u32| {
        function[(row * size + col) as usize] = true;
    };

    // Finder patterns and their separators.
    for row in 0..size {
        for col in 0..size {
            if finder_corner(size, row, col).is_some() {
                mark(&mut function, row, col);
            }
        }
    }

    // Timing patterns.
    for index in 8..size - 8 {
        mark(&mut function, 6, index);
        mark(&mut function, index, 6);
    }

    // Alignment patterns, skipping the three finder corners.
    let positions = alignment_positions(version);
    let count = positions.len();
    if count > 0 {
        for i in 0..count {
            for j in 0..count {
                if (i == 0 && j == 0) || (i == 0 && j == count - 1) || (i == count - 1 && j == 0) {
                    continue;
                }
                let center_row = positions[i];
                let center_col = positions[j];
                for delta_row in -2i32..=2 {
                    for delta_col in -2i32..=2 {
                        let row = (center_row as i32 + delta_row) as u32;
                        let col = (center_col as i32 + delta_col) as u32;
                        mark(&mut function, row, col);
                    }
                }
            }
        }
    }

    // Dark module.
    mark(&mut function, size - 8, 8);

    // Format information, both copies.
    for index in 0..6 {
        mark(&mut function, index, 8);
    }
    mark(&mut function, 7, 8);
    mark(&mut function, 8, 8);
    mark(&mut function, 8, 7);
    for index in 9..15 {
        mark(&mut function, 8, 14 - index);
    }
    for index in 0..8 {
        mark(&mut function, 8, size - 1 - index);
    }
    for index in 8..15 {
        mark(&mut function, size - 15 + index, 8);
    }

    // Version information, both copies, for versions 7 and above.
    if version >= 7 {
        for index in 0..18u32 {
            let a = size - 11 + index % 3;
            let b = index / 3;
            mark(&mut function, b, a);
            mark(&mut function, a, b);
        }
    }

    function
}

/// Draws the three finder patterns and their light separators.
fn draw_finders(matrix: &mut QrMatrix) {
    let size = matrix.size;
    for row in 0..size {
        for col in 0..size {
            let Some((base_row, base_col, flip_row, flip_col)) = finder_corner(size, row, col)
            else {
                continue;
            };

            let mut local_row = row - base_row;
            let mut local_col = col - base_col;
            if flip_row {
                local_row = FINDER_SIZE - local_row;
            }
            if flip_col {
                local_col = FINDER_SIZE - local_col;
            }

            // The separator is the eighth row/column of the corner block.
            let dark = if local_row == 7 || local_col == 7 {
                false
            } else {
                let outer = local_row == 0 || local_row == 6 || local_col == 0 || local_col == 6;
                let inner = (2..=4).contains(&local_row) && (2..=4).contains(&local_col);
                outer || inner
            };
            matrix.set(row, col, dark);
        }
    }
}

/// Draws the horizontal and vertical timing patterns.
fn draw_timing(matrix: &mut QrMatrix) {
    let size = matrix.size;
    for index in 8..size - 8 {
        let dark = index % 2 == 0;
        matrix.set(6, index, dark);
        matrix.set(index, 6, dark);
    }
}

/// Alignment-pattern centre coordinates for each supported version.
///
/// Versions 1–40 are tabulated rather than derived: the positions are a published table and
/// deriving them would put a second, subtly different rule beside it.
fn alignment_positions(version: u8) -> &'static [u32] {
    match version {
        1 => &[],
        2 => &[6, 18],
        3 => &[6, 22],
        4 => &[6, 26],
        5 => &[6, 30],
        6 => &[6, 34],
        7 => &[6, 22, 38],
        8 => &[6, 24, 42],
        9 => &[6, 26, 46],
        10 => &[6, 28, 50],
        11 => &[6, 30, 54],
        12 => &[6, 32, 58],
        13 => &[6, 34, 62],
        14 => &[6, 26, 46, 66],
        15 => &[6, 26, 48, 70],
        16 => &[6, 26, 50, 74],
        17 => &[6, 30, 54, 78],
        18 => &[6, 30, 56, 82],
        19 => &[6, 30, 58, 86],
        20 => &[6, 34, 62, 90],
        21 => &[6, 28, 50, 72, 94],
        22 => &[6, 26, 50, 74, 98],
        23 => &[6, 30, 54, 78, 102],
        24 => &[6, 28, 54, 80, 106],
        25 => &[6, 32, 58, 84, 110],
        26 => &[6, 30, 58, 86, 114],
        27 => &[6, 34, 62, 90, 118],
        28 => &[6, 26, 50, 74, 98, 122],
        29 => &[6, 30, 54, 78, 102, 126],
        30 => &[6, 26, 52, 78, 104, 130],
        31 => &[6, 30, 56, 82, 108, 134],
        32 => &[6, 34, 60, 86, 112, 138],
        33 => &[6, 30, 58, 86, 114, 142],
        34 => &[6, 34, 62, 90, 118, 146],
        35 => &[6, 30, 54, 78, 102, 126, 150],
        36 => &[6, 24, 50, 76, 102, 128, 154],
        37 => &[6, 28, 54, 80, 106, 132, 158],
        38 => &[6, 32, 58, 84, 110, 136, 162],
        39 => &[6, 26, 54, 82, 110, 138, 166],
        40 => &[6, 30, 58, 86, 114, 142, 170],
        _ => &[],
    }
}

/// Draws every alignment pattern for a version.
fn draw_alignment_patterns(matrix: &mut QrMatrix, version: u8) {
    let positions = alignment_positions(version);
    let count = positions.len();
    if count == 0 {
        return;
    }
    for i in 0..count {
        for j in 0..count {
            // The three finder corners already occupy these positions.
            if (i == 0 && j == 0) || (i == 0 && j == count - 1) || (i == count - 1 && j == 0) {
                continue;
            }
            draw_alignment_pattern(matrix, positions[i], positions[j]);
        }
    }
}

/// Draws one 5×5 alignment pattern: dark border, light ring, dark centre.
fn draw_alignment_pattern(matrix: &mut QrMatrix, center_row: u32, center_col: u32) {
    for delta_row in -2i32..=2 {
        for delta_col in -2i32..=2 {
            let distance = delta_row.abs().max(delta_col.abs());
            let dark = distance != 1;
            let row = (center_row as i32 + delta_row) as u32;
            let col = (center_col as i32 + delta_col) as u32;
            matrix.set(row, col, dark);
        }
    }
}

/// Draws the 18-bit version information for versions 7 and above.
fn draw_version_info(matrix: &mut QrMatrix, version: u8) {
    let bits = version_bits(version);
    let size = matrix.size;
    for index in 0..18u32 {
        let dark = (bits >> index) & 1 != 0;
        let a = size - 11 + index % 3;
        let b = index / 3;
        matrix.set(b, a, dark);
        matrix.set(a, b, dark);
    }
}

/// BCH(18, 6) version information with the standard `0x1F25` generator.
fn version_bits(version: u8) -> u32 {
    let mut remainder = u32::from(version);
    for _ in 0..12 {
        remainder = (remainder << 1) ^ (((remainder >> 11) & 1) * 0x1F25);
    }
    (u32::from(version) << 12) | remainder
}

/// BCH(15, 5) format information with the standard `0x537` generator and `0x5412` mask.
fn format_bits(ec: QrEcLevel, mask: u8) -> u16 {
    let data = (u16::from(ec.format_bits()) << 3) | u16::from(mask & 0x07);
    let mut remainder = data;
    for _ in 0..10 {
        remainder = (remainder << 1) ^ (((remainder >> 9) & 1) * 0x537);
    }
    ((data << 10) | remainder) ^ 0x5412
}

/// Writes both copies of the format information, plus the always-dark module.
fn draw_format_info(matrix: &mut QrMatrix, ec: QrEcLevel, mask: u8) {
    let bits = format_bits(ec, mask);
    let size = matrix.size;

    // First copy, around the top-left finder.
    for index in 0..6u32 {
        matrix.set(index, 8, (bits >> index) & 1 != 0);
    }
    matrix.set(7, 8, (bits >> 6) & 1 != 0);
    matrix.set(8, 8, (bits >> 7) & 1 != 0);
    matrix.set(8, 7, (bits >> 8) & 1 != 0);
    for index in 9..15u32 {
        matrix.set(8, 14 - index, (bits >> index) & 1 != 0);
    }

    // Second copy, split across the bottom and right edges.
    for index in 0..8u32 {
        matrix.set(8, size - 1 - index, (bits >> index) & 1 != 0);
    }
    for index in 8..15u32 {
        matrix.set(size - 15 + index, 8, (bits >> index) & 1 != 0);
    }

    matrix.set(size - 8, 8, true);
}

/// Places the interleaved codeword bits in the standard upward/downward zigzag.
fn place_data(matrix: &mut QrMatrix, function: &[bool], codewords: &[u8]) {
    let size = matrix.size as i32;
    let total_bits = codewords.len() * 8;
    let mut bit_index = 0usize;
    let mut right = size - 1;

    while right > 0 {
        // Column 6 is the vertical timing pattern; shift the pair left past it.
        if right == 6 {
            right = 5;
        }
        for vertical in 0..size {
            for offset in 0..2 {
                let col = right - offset;
                let upward = ((right + 1) & 2) == 0;
                let row = if upward { size - 1 - vertical } else { vertical };
                let index = (row * size + col) as usize;
                if !function[index] && bit_index < total_bits {
                    let dark = (codewords[bit_index >> 3] >> (7 - (bit_index & 7))) & 1 != 0;
                    matrix.modules[index] = dark;
                    bit_index += 1;
                }
            }
        }
        right -= 2;
    }
}

/// Tests whether a mask condition applies at `(row, col)`.
fn mask_condition(mask: u8, row: u32, col: u32) -> bool {
    match mask {
        0 => (row + col).is_multiple_of(2),
        1 => row.is_multiple_of(2),
        2 => col.is_multiple_of(3),
        3 => (row + col).is_multiple_of(3),
        4 => (row / 2 + col / 3).is_multiple_of(2),
        5 => (row * col) % 2 + (row * col) % 3 == 0,
        6 => ((row * col) % 2 + (row * col) % 3).is_multiple_of(2),
        7 => ((row + col) % 2 + (row * col) % 3).is_multiple_of(2),
        _ => false,
    }
}

/// XORs a mask over every non-function module.
fn apply_mask(matrix: &mut QrMatrix, function: &[bool], mask: u8) {
    for row in 0..matrix.size {
        for col in 0..matrix.size {
            let index = (row * matrix.size + col) as usize;
            if !function[index] && mask_condition(mask, row, col) {
                matrix.modules[index] = !matrix.modules[index];
            }
        }
    }
}

/// The 1:1:3:1:1 finder-like pattern followed by four light modules.
const FINDER_PENALTY_PATTERN: [bool; 11] =
    [true, false, true, true, true, false, true, false, false, false, false];

/// The mirror form: four light modules followed by the 1:1:3:1:1 pattern.
const FINDER_PENALTY_PATTERN_REVERSED: [bool; 11] =
    [false, false, false, false, true, false, true, true, true, false, true];

/// Applies the four standard mask penalty rules.
fn penalty(matrix: &QrMatrix) -> u32 {
    let size = matrix.size as i32;
    let mut score = 0u32;

    // Rule 1: runs of five or more same-coloured modules in a row or column.
    for index in 0..size {
        let mut run_colour = matrix.get(index as u32, 0);
        let mut run_length = 1u32;
        for col in 1..size {
            let colour = matrix.get(index as u32, col as u32);
            if colour == run_colour {
                run_length += 1;
            } else {
                if run_length >= 5 {
                    score += 3 + (run_length - 5);
                }
                run_colour = colour;
                run_length = 1;
            }
        }
        if run_length >= 5 {
            score += 3 + (run_length - 5);
        }

        let mut run_colour = matrix.get(0, index as u32);
        let mut run_length = 1u32;
        for row in 1..size {
            let colour = matrix.get(row as u32, index as u32);
            if colour == run_colour {
                run_length += 1;
            } else {
                if run_length >= 5 {
                    score += 3 + (run_length - 5);
                }
                run_colour = colour;
                run_length = 1;
            }
        }
        if run_length >= 5 {
            score += 3 + (run_length - 5);
        }
    }

    // Rule 2: every 2×2 block of one colour.
    for row in 0..size - 1 {
        for col in 0..size - 1 {
            let colour = matrix.get(row as u32, col as u32);
            if colour == matrix.get((row + 1) as u32, col as u32)
                && colour == matrix.get(row as u32, (col + 1) as u32)
                && colour == matrix.get((row + 1) as u32, (col + 1) as u32)
            {
                score += 3;
            }
        }
    }

    // Rule 3: finder-like 1:1:3:1:1 patterns with four light modules on one side.
    let mut window = [false; 11];
    for index in 0..size {
        for start in 0..=size - 11 {
            for offset in 0..11 {
                window[offset as usize] = matrix.get(index as u32, (start + offset) as u32);
            }
            if window == FINDER_PENALTY_PATTERN || window == FINDER_PENALTY_PATTERN_REVERSED {
                score += 40;
            }

            for offset in 0..11 {
                window[offset as usize] = matrix.get((start + offset) as u32, index as u32);
            }
            if window == FINDER_PENALTY_PATTERN || window == FINDER_PENALTY_PATTERN_REVERSED {
                score += 40;
            }
        }
    }

    // Rule 4: deviation of the dark-module proportion from 50%, in 5% steps.
    let dark = matrix.modules.iter().filter(|&&dark| dark).count() as i64;
    let total = i64::from(matrix.size) * i64::from(matrix.size);
    let deviation = (dark * 20 - total * 10).unsigned_abs() / total as u64;
    score += (deviation as u32) * 10;

    score
}

/// Reads the first copy of the 15-bit format information from `matrix`.
fn read_format_bits(matrix: &QrMatrix) -> u16 {
    let mut bits = 0u16;
    for index in 0..6u32 {
        if matrix.get(index, 8) {
            bits |= 1u16 << index;
        }
    }
    if matrix.get(7, 8) {
        bits |= 1 << 6;
    }
    if matrix.get(8, 8) {
        bits |= 1 << 7;
    }
    if matrix.get(8, 7) {
        bits |= 1 << 8;
    }
    for index in 9..15u32 {
        if matrix.get(8, 14 - index) {
            bits |= 1u16 << index;
        }
    }
    bits
}

/// Reads the EC level from the format information already present in `matrix`.
///
/// `1` is level L and `0` is level M. Q and H are not supported; a value that does not
/// decode to L is treated as M for scoring, which is harmless because the caller rewrites the
/// format information with the level it actually used.
fn decode_ec_level(matrix: &QrMatrix) -> QrEcLevel {
    let unmasked = read_format_bits(matrix) ^ 0x5412;
    match (unmasked >> 13) & 0x03 {
        0b01 => QrEcLevel::L,
        0b11 => QrEcLevel::Q,
        0b10 => QrEcLevel::H,
        _ => QrEcLevel::M,
    }
}

/// Chooses the mask with the lowest penalty score (lowest index wins ties).
///
/// The format information is part of the symbol and therefore part of the four penalty
/// rules, so each candidate is scored with its own format bits in place. The caller must have
/// written a provisional format field before calling this; the provisional EC level is read
/// back from it and reused for every candidate.
fn best_mask(matrix: &QrMatrix) -> u8 {
    let version = matrix_version(matrix.size);
    let function = function_modules(matrix.size, version);
    let ec = decode_ec_level(matrix);
    let mut chosen = 0u8;
    let mut best_score = u32::MAX;

    for mask in 0..8u8 {
        let mut candidate = matrix.clone();
        apply_mask(&mut candidate, &function, mask);
        draw_format_info(&mut candidate, ec, mask);
        let score = penalty(&candidate);
        if score < best_score {
            best_score = score;
            chosen = mask;
        }
    }

    chosen
}

/// Encodes `data` as a byte-mode QR symbol, choosing the smallest fitting version.
///
/// Test-only constructor for the default version window.
#[cfg(test)]
fn encode_byte_mode(data: &[u8], ec: QrEcLevel) -> Result<QrMatrix, QrEncodeError> {
    encode_byte_mode_in(data, ec, 1, 40)
}

/// Encodes `data` within an explicit version window.
fn encode_byte_mode_in(
    data: &[u8],
    ec: QrEcLevel,
    min_version: u8,
    max_version: u8,
) -> Result<QrMatrix, QrEncodeError> {
    let version = choose_version_in(data.len(), ec, min_version, max_version)?;
    let spec = block_spec(version, ec).ok_or_else(|| {
        QrEncodeError(format!(
            "no Reed-Solomon block layout for QR version {version} at EC level {ec:?}"
        ))
    })?;
    let codewords = build_codewords(data, version, spec)?;

    let (mut matrix, function) = build_base_matrix(version);
    place_data(&mut matrix, &function, &codewords);

    // Write a provisional format field: `best_mask` reads the EC level back from it and scores
    // each candidate as a complete symbol, format bits included.
    draw_format_info(&mut matrix, ec, 0);
    let mask = best_mask(&matrix);
    apply_mask(&mut matrix, &function, mask);
    draw_format_info(&mut matrix, ec, mask);

    Ok(matrix)
}

/// A QR segment mode.
///
/// The byte-mode-only encoder forced every payload into 8 bits per character, so a numeric string
/// cost more than three times what it needed (10 bits per *three* digits in numeric mode) and a
/// mixed payload always chose a larger version than the data required (MISC-13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum QrMode {
    /// `0001`: three digits per 10 bits.
    Numeric,
    /// `0010`: two characters from the 45-symbol alphanumeric set per 11 bits.
    AlphaNumeric,
    /// `0100`: eight bits per byte, the historical mode and the universal fallback.
    Byte,
}

/// The 45 characters alphanumeric mode can encode, in their QR index order.
const ALNUM_CHARSET: &[u8; 45] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ $%*+-./:";

impl QrMode {
    /// The four-bit mode indicator.
    fn indicator(self) -> u32 {
        match self {
            QrMode::Numeric => 0b0001,
            QrMode::AlphaNumeric => 0b0010,
            QrMode::Byte => 0b0100,
        }
    }

    /// The width of this mode's character-count indicator at `version`.
    fn count_bits(self, version: u8) -> usize {
        match (self, version <= 9, version <= 26) {
            (QrMode::Numeric, true, _) => 10,
            (QrMode::Numeric, false, true) => 12,
            (QrMode::Numeric, false, false) => 14,
            (QrMode::AlphaNumeric, true, _) => 9,
            (QrMode::AlphaNumeric, false, true) => 11,
            (QrMode::AlphaNumeric, false, false) => 13,
            (QrMode::Byte, true, _) => 8,
            (QrMode::Byte, false, _) => 16,
        }
    }

    /// The payload bits `len` characters occupy in this mode.
    fn payload_bits(self, len: usize) -> usize {
        match self {
            QrMode::Numeric => 10 * (len / 3) + [0, 4, 7][len % 3],
            QrMode::AlphaNumeric => 11 * (len / 2) + 6 * (len % 2),
            QrMode::Byte => 8 * len,
        }
    }
}

/// The alphanumeric-mode value of `byte`, or `None` when it is outside the charset.
fn alnum_value(byte: u8) -> Option<u8> {
    ALNUM_CHARSET.iter().position(|candidate| *candidate == byte).map(|index| index as u8)
}

/// One planned segment: a mode plus the byte range it covers.
type QrSegment = (QrMode, usize, usize);

/// Plans the segment list for `data` at `version`.
///
/// Consecutive digits become a numeric run and consecutive charset characters an alphanumeric run;
/// everything else is a byte run. The plan is then compared with a single byte-mode segment and the
/// byte form wins ties, so this can only ever make a payload more compact — a short numeric run
/// whose extra mode header costs more than it saves stays in byte mode.
fn plan_segments(data: &[u8], version: u8) -> Vec<QrSegment> {
    let byte_total = 4 + QrMode::Byte.count_bits(version) + QrMode::Byte.payload_bits(data.len());
    let mut segments: Vec<QrSegment> = Vec::new();
    let mut index = 0usize;
    while index < data.len() {
        let byte = data[index];
        let mode = if byte.is_ascii_digit() {
            QrMode::Numeric
        } else if alnum_value(byte).is_some() {
            QrMode::AlphaNumeric
        } else {
            QrMode::Byte
        };
        let start = index;
        match mode {
            QrMode::Numeric => {
                while index < data.len() && data[index].is_ascii_digit() {
                    index += 1;
                }
            }
            QrMode::AlphaNumeric => {
                while index < data.len()
                    && !data[index].is_ascii_digit()
                    && alnum_value(data[index]).is_some()
                {
                    index += 1;
                }
            }
            QrMode::Byte => {
                while index < data.len()
                    && !data[index].is_ascii_digit()
                    && alnum_value(data[index]).is_none()
                {
                    index += 1;
                }
            }
        }
        segments.push((mode, start, index - start));
    }

    let segmented_total: usize = segments
        .iter()
        .map(|(mode, _, len)| 4 + mode.count_bits(version) + mode.payload_bits(*len))
        .sum();
    if segmented_total >= byte_total {
        return vec![(QrMode::Byte, 0, data.len())];
    }
    segments
}

/// Encodes `data`, auto-segmenting into numeric / alphanumeric / byte runs, smallest version first.
fn encode_segmented_in(
    data: &[u8],
    ec: QrEcLevel,
    min_version: u8,
    max_version: u8,
) -> Result<QrMatrix, QrEncodeError> {
    let min_version = min_version.clamp(1, 40);
    let max_version = max_version.clamp(min_version, 40);
    let mut chosen = None;
    for version in min_version..=max_version {
        let Some(spec) = block_spec(version, ec) else {
            continue;
        };
        let plan = plan_segments(data, version);
        let needed_bits: usize = plan
            .iter()
            .map(|(mode, _, len)| 4 + mode.count_bits(version) + mode.payload_bits(*len))
            .sum();
        // A count indicator can only describe as many characters as it has bits for.
        let counts_fit =
            plan.iter().all(|(mode, _, len)| *len < (1usize << mode.count_bits(version)));
        if counts_fit && needed_bits.div_ceil(8) <= spec.data_codewords {
            chosen = Some((version, spec, plan));
            break;
        }
    }
    let Some((version, spec, plan)) = chosen else {
        return Err(QrEncodeError(format!(
            "payload of {} bytes does not fit any supported QR version ({}..={}) at EC level \
             {ec:?}",
            data.len(),
            min_version,
            max_version
        )));
    };
    // A payload with nothing to compress plans as one byte segment, so it takes the plain byte-mode
    // encoder — the same code path, and therefore the same symbol, the control had before the
    // segmented encoder existed. That also keeps one symbol shape per payload rather than two
    // equivalent ones.
    if plan.len() == 1 && plan[0].0 == QrMode::Byte {
        return encode_byte_mode_in(data, ec, min_version, max_version);
    }

    let capacity_bits = spec.data_codewords * 8;
    let mut bits = Vec::with_capacity(capacity_bits);
    for (mode, start, len) in &plan {
        push_bits(&mut bits, mode.indicator(), 4);
        push_bits(&mut bits, *len as u32, mode.count_bits(version));
        let run = &data[*start..*start + *len];
        match mode {
            QrMode::Numeric => {
                for chunk in run.chunks(3) {
                    let mut value = 0u32;
                    for byte in chunk {
                        value = value * 10 + u32::from(byte - b'0');
                    }
                    let width = match chunk.len() {
                        3 => 10,
                        2 => 7,
                        _ => 4,
                    };
                    push_bits(&mut bits, value, width);
                }
            }
            QrMode::AlphaNumeric => {
                for pair in run.chunks(2) {
                    let high = u32::from(alnum_value(pair[0]).unwrap_or(0));
                    let value = if pair.len() == 2 {
                        high * 45 + u32::from(alnum_value(pair[1]).unwrap_or(0))
                    } else {
                        high
                    };
                    push_bits(&mut bits, value, if pair.len() == 2 { 11 } else { 6 });
                }
            }
            QrMode::Byte => {
                for byte in run {
                    push_bits(&mut bits, u32::from(*byte), 8);
                }
            }
        }
    }

    if bits.len() > capacity_bits {
        return Err(QrEncodeError(format!(
            "segmented bit stream is {} bits but the selected QR version only holds {}",
            bits.len(),
            capacity_bits
        )));
    }
    let codewords = assemble_codewords(bits, spec)?;

    let (mut matrix, function) = build_base_matrix(version);
    place_data(&mut matrix, &function, &codewords);
    draw_format_info(&mut matrix, ec, 0);
    let mask = best_mask(&matrix);
    apply_mask(&mut matrix, &function, mask);
    draw_format_info(&mut matrix, ec, mask);
    Ok(matrix)
}

/// Galois-field multiplication in GF(256) with primitive polynomial `0x11D`.
fn gf_mul(left: u8, right: u8) -> u8 {
    if left == 0 || right == 0 {
        return 0;
    }
    let exponent = usize::from(GF_LOG[left as usize]) + usize::from(GF_LOG[right as usize]);
    GF_EXP[exponent]
}

/// Builds the exponent and logarithm tables for GF(256).
const fn gf_tables() -> ([u8; 512], [u8; 256]) {
    let mut exp = [0u8; 512];
    let mut log = [0u8; 256];
    let mut value: u16 = 1;
    let mut exponent = 0usize;
    while exponent < 255 {
        exp[exponent] = value as u8;
        log[value as usize] = exponent as u8;
        value <<= 1;
        if value & 0x100 != 0 {
            value ^= 0x11D;
        }
        exponent += 1;
    }
    let mut index = 255usize;
    while index < 512 {
        exp[index] = exp[index - 255];
        index += 1;
    }
    (exp, log)
}

/// `(exponent table, logarithm table)` for GF(256).
const GF_TABLES: ([u8; 512], [u8; 256]) = gf_tables();

/// The exponent table: `GF_EXP[e] == α^e`, duplicated so sums up to 508 are in range.
const GF_EXP: [u8; 512] = GF_TABLES.0;

/// The logarithm table: `GF_LOG[α^e] == e`, with `GF_LOG[0] == 0` unused.
const GF_LOG: [u8; 256] = GF_TABLES.1;

/// Builds the degree-`degree` Reed–Solomon generator polynomial `(x - α^0) ...`.
///
/// The returned coefficients omit the leading 1 and are ordered from the highest power down.
fn rs_generator(degree: usize) -> Vec<u8> {
    let mut coefficients = vec![0u8; degree];
    coefficients[degree - 1] = 1;
    let mut root = 1u8;
    for _ in 0..degree {
        for index in 0..degree {
            coefficients[index] = gf_mul(coefficients[index], root);
            if index + 1 < degree {
                coefficients[index] ^= coefficients[index + 1];
            }
        }
        root = gf_mul(root, 0x02);
    }
    coefficients
}

/// Appends `ec_codewords` Reed–Solomon parity codewords to `data`.
fn reed_solomon_append(data: &[u8], ec_codewords: usize) -> Vec<u8> {
    let mut result = data.to_vec();
    if ec_codewords == 0 {
        return result;
    }

    let coefficients = rs_generator(ec_codewords);
    let mut remainder = vec![0u8; ec_codewords];
    for &byte in data {
        let factor = byte ^ remainder[0];
        remainder.copy_within(1.., 0);
        remainder[ec_codewords - 1] = 0;
        for (slot, &coefficient) in remainder.iter_mut().zip(coefficients.iter()) {
            *slot ^= gf_mul(coefficient, factor);
        }
    }

    result.extend_from_slice(&remainder);
    result
}

/// QRCode widget that renders a real byte-mode QR symbol.
pub struct QRCode {
    base: BaseWidget,
    data: String,
    /// Size of each module (cell) in logical pixels.
    module_size: u32,
    /// Quiet zone (white border) around the matrix in modules.
    quiet_zone: u32,
    /// The encoded symbol. It is materialised by `set_data` instead of being regenerated on
    /// every painted frame.
    matrix: QrMatrix,
    /// The reason the current `data` could not be encoded, if any. When set, `draw` renders an
    /// explicit error placeholder and never a fake code.
    encode_error: Option<String>,
    /// Error-correction level used by the encoder.
    ec_level: QrEcLevel,
    /// Smallest QR version the encoder may choose.
    min_version: u8,
    /// Largest QR version the encoder may choose.
    max_version: u8,
    /// Caller-supplied module colour, or `None` to follow the theme's ink.
    foreground_color: Option<Color>,
    /// Caller-supplied quiet-zone colour, or `None` to follow the theme's background.
    background_color: Option<Color>,
    /// An optional mark painted in a cut-out at the symbol's centre.
    ///
    /// `None` is the historical symbol: every module is painted. When set, the modules under the
    /// centred square are treated as light (the "hole") and the mark is painted on top, which is
    /// the only way a decoder can tolerate a logo (MISC-03).
    logo: Option<IconSource>,
    /// Side of the logo hole in modules; `0` selects the default (a fifth of the symbol).
    logo_modules: u32,
}

impl QRCode {
    /// Creates a new QRCode widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        let mut widget = Self {
            base: BaseWidget::new(WidgetKind::QRCode, geometry, "QRCode"),
            data: String::new(),
            module_size: 4,
            quiet_zone: 2,
            matrix: QrMatrix::blank(matrix_size(1)),
            encode_error: None,
            ec_level: QrEcLevel::L,
            min_version: 1,
            max_version: 40,
            foreground_color: None,
            background_color: None,
            logo: None,
            logo_modules: 0,
        };
        widget.reencode();
        widget
    }

    /// Returns a reference to the current data string.
    pub fn data(&self) -> &str {
        &self.data
    }

    /// Returns why the current data could not be encoded, if the last encode failed.
    pub fn encode_error(&self) -> Option<&str> {
        self.encode_error.as_deref()
    }

    /// Sets the data string used to generate the QR code pattern.
    pub fn set_data(&mut self, data: &str) {
        if self.data == data {
            return;
        }
        self.data = data.to_string();
        self.reencode();
        self.base.request_redraw();
    }

    /// Re-encodes `data`, recording an error instead of keeping a stale or fabricated matrix.
    ///
    /// The encoder picks numeric / alphanumeric / byte segments per run (MISC-13), so a numeric or
    /// uppercase payload occupies the version its own content needs rather than the byte-mode
    /// worst case. Ties go to byte mode, so nothing that used to fit stops fitting.
    fn reencode(&mut self) {
        match encode_segmented_in(
            self.data.as_bytes(),
            self.ec_level,
            self.min_version,
            self.max_version,
        ) {
            Ok(matrix) => {
                self.matrix = matrix;
                self.encode_error = None;
            }
            Err(error) => {
                self.matrix = QrMatrix::blank(matrix_size(1));
                self.encode_error = Some(error.0);
            }
        }
    }

    /// Returns the current module size in pixels.
    pub fn module_size(&self) -> u32 {
        self.module_size
    }

    /// The error-correction level currently used.
    pub fn ec_level(&self) -> QrEcLevel {
        self.ec_level
    }

    /// Sets the error-correction level and re-encodes.
    pub fn set_ec_level(&mut self, level: QrEcLevel) {
        if self.ec_level == level {
            return;
        }
        self.ec_level = level;
        self.reencode();
        self.base.request_redraw();
    }

    /// The version window the encoder may choose from.
    pub fn version_range(&self) -> (u8, u8) {
        (self.min_version, self.max_version)
    }

    /// Sets the version window (clamped to the specification's `1..=40`) and re-encodes.
    pub fn set_version_range(&mut self, min_version: u8, max_version: u8) {
        let min_version = min_version.clamp(1, 40);
        let max_version = max_version.clamp(min_version, 40);
        if self.min_version == min_version && self.max_version == max_version {
            return;
        }
        self.min_version = min_version;
        self.max_version = max_version;
        self.reencode();
        self.base.request_redraw();
    }

    /// The caller-supplied module colour, if any.
    pub fn foreground_color(&self) -> Option<Color> {
        self.foreground_color
    }

    /// Sets the module colour; `None` follows the theme.
    pub fn set_foreground_color(&mut self, color: Option<Color>) {
        if self.foreground_color == color {
            return;
        }
        self.foreground_color = color;
        self.base.request_redraw();
    }

    /// The caller-supplied quiet-zone colour, if any.
    pub fn background_color(&self) -> Option<Color> {
        self.background_color
    }

    /// Sets the quiet-zone colour; `None` follows the theme.
    pub fn set_background_color(&mut self, color: Option<Color>) {
        if self.background_color == color {
            return;
        }
        self.background_color = color;
        self.base.request_redraw();
    }

    /// The optional centre mark, or `None` for the plain symbol.
    pub fn logo(&self) -> Option<IconSource> {
        self.logo
    }

    /// Installs or removes the centre mark.
    ///
    /// The mark is painted in a light cut-out rather than over the modules: a logo that leaves
    /// the modules beneath it intact is invisible to the decoder, so the cut-out is the feature
    /// that makes a logo usable rather than the picture itself. Nothing about the encoding
    /// changes, so this does not re-encode.
    pub fn set_logo(&mut self, logo: Option<IconSource>) {
        if self.logo == logo {
            return;
        }
        self.logo = logo;
        self.base.request_redraw();
    }

    /// The configured cut-out side in modules; `0` means the default of a fifth of the symbol.
    pub fn logo_size_modules(&self) -> u32 {
        self.logo_modules
    }

    /// Sets the cut-out side in modules; `0` restores the automatic size.
    pub fn set_logo_size_modules(&mut self, modules: u32) {
        if self.logo_modules == modules {
            return;
        }
        self.logo_modules = modules;
        self.base.request_redraw();
    }

    /// The error-correction level a logo-carrying symbol should use, if any.
    ///
    /// A logo removes data from the symbol, so the level has to leave enough recovery budget for
    /// it. Level **H** recovers about 30% of codewords, which covers the default hole (about 4%
    /// of the modules) with a wide margin. The hint is advisory: [`QRCode::set_ec_level`] is not
    /// called here, so a caller that never asks for a logo keeps the historical level.
    pub fn logo_ec_hint(&self) -> Option<QrEcLevel> {
        self.logo.map(|_| QrEcLevel::H)
    }

    /// The centred module square reserved for the logo, as `(row, column, side)`.
    ///
    /// `None` while no logo is installed. The square is clamped to leave the finder patterns and
    /// their separators intact, and is centred on the symbol. A caller-sized square is honoured
    /// up to that limit, so a badly large value cannot erase the patterns a scanner relies on.
    fn logo_hole(&self) -> Option<(u32, u32, u32)> {
        self.logo?;
        let size = self.matrix.size;
        let requested = if self.logo_modules == 0 { (size / 5).max(5) } else { self.logo_modules };
        let side = requested.min(size.saturating_sub(8)).max(1);
        if side == 0 {
            return None;
        }
        let origin = (size - side) / 2;
        Some((origin, origin, side))
    }

    /// Whether `(row, column)` falls inside the logo cut-out.
    #[cfg(test)]
    fn is_logo_module(&self, row: u32, col: u32) -> bool {
        match self.logo_hole() {
            Some((origin_row, origin_col, side)) => {
                row >= origin_row
                    && row < origin_row + side
                    && col >= origin_col
                    && col < origin_col + side
            }
            None => false,
        }
    }

    /// Returns the size of the **quiet zone** — the light margin around the symbol — in
    /// modules.
    ///
    /// The quiet zone is a first-class parameter of a QR symbol, not decoration: the
    /// specification requires a margin of at least four modules for a decoder to locate the
    /// finder patterns, and both `qrencode`'s `margin` and every other QR library expose it. This
    /// control had the field and drew with it, but published no way to read or set it, so a
    /// caller could not satisfy a scanner that needed a wider margin.
    pub fn quiet_zone(&self) -> u32 {
        self.quiet_zone
    }

    /// Sets the quiet-zone width in modules.
    ///
    /// Zero is accepted and means "no margin" — legitimate when the caller is composing the
    /// symbol into a layout that supplies its own padding. The value is stored as given rather
    /// than clamped to the specification's minimum four: this control draws a symbol, and
    /// refusing a margin a caller explicitly asked for would be a different kind of wrong. The
    /// default is two, unchanged, so existing renders are unaffected.
    pub fn set_quiet_zone(&mut self, modules: u32) {
        if self.quiet_zone == modules {
            return;
        }
        self.quiet_zone = modules;
        self.base.request_redraw();
    }

    /// Returns the rendered side length of the whole symbol in pixels,
    /// including the quiet zone on both edges.
    pub fn size(&self) -> u32 {
        (self.matrix.size + self.quiet_zone * 2) * self.module_size
    }

    /// Sets the size of each module (cell) in logical pixels.
    pub fn set_module_size(&mut self, size: u32) {
        let size = size.max(1);
        if self.module_size == size {
            return;
        }
        self.module_size = size;
        self.base.request_redraw();
    }
}

impl Widget for QRCode {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        let side = crate::widget::metrics::dimensions::QR_CODE_SIZE;
        crate::core::Size::new(side, side)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `QRCode`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `size` reports the rendered
/// symbol extent in pixels; a write is carried to the module size, which is the
/// only knob the matrix has, and the value then reads back rounded to a whole
/// number of modules. Negative writes are clamped to one pixel per module rather
/// than rejected, matching `set_module_size`.
impl WidgetProperties for QRCode {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "data" => Ok(CapabilityValue::String(self.data().to_string())),
            "size" => Ok(CapabilityValue::UInt(self.size() as u64)),
            "quiet_zone" => Ok(CapabilityValue::UInt(self.quiet_zone() as u64)),
            "ec_level" => Ok(CapabilityValue::String(self.ec_level.as_str().to_string())),
            "min_version" => Ok(CapabilityValue::UInt(u64::from(self.min_version))),
            "max_version" => Ok(CapabilityValue::UInt(u64::from(self.max_version))),
            "foreground_color" => Ok(match self.foreground_color {
                Some(c) => CapabilityValue::Color(c),
                None => CapabilityValue::Null,
            }),
            "background_color" => Ok(match self.background_color {
                Some(c) => CapabilityValue::Color(c),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "data" => {
                self.set_data(&expect_string(value)?);
                Ok(())
            }
            "size" => {
                let requested = expect_u32_saturating(value)?;
                let module = (requested / (self.matrix.size + self.quiet_zone * 2)).max(1);
                self.set_module_size(module);
                Ok(())
            }
            "quiet_zone" => {
                self.set_quiet_zone(expect_u32_saturating(value)?);
                Ok(())
            }
            "ec_level" => {
                let token = expect_string(value)?;
                let level =
                    QrEcLevel::from_token(&token).ok_or(CapabilityAccessError::TypeMismatch)?;
                self.set_ec_level(level);
                Ok(())
            }
            "min_version" => {
                let min = expect_usize(value)?.min(40) as u8;
                self.set_version_range(min, self.max_version);
                Ok(())
            }
            "max_version" => {
                let max = expect_usize(value)?.clamp(1, 40) as u8;
                self.set_version_range(self.min_version, max);
                Ok(())
            }
            "foreground_color" => {
                match value {
                    CapabilityValue::Null => self.set_foreground_color(None),
                    CapabilityValue::Color(color) => self.set_foreground_color(Some(color)),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            "background_color" => {
                match value {
                    CapabilityValue::Null => self.set_background_color(None),
                    CapabilityValue::Color(color) => self.set_background_color(Some(color)),
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "data",
            "size",
            "quiet_zone",
            "ec_level",
            "min_version",
            "max_version",
            "foreground_color",
            "background_color",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for QRCode {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();

        // The symbol is a contrasting module/surface pair, and *both* members come
        // from the theme: modules are the resolved text colour, the quiet zone is the
        // resolved background. That keeps the symbol scannable in either appearance
        // while making it respond to a theme switch; hardcoding white/black meant the
        // quiet zone stayed white on a dark window.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("qr_code");
        let surface = self
            .background_color
            .or(style.background_color)
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::WHITE);
        let module_color = self
            .foreground_color
            .or(style.text_color)
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(module_color);

        // A failed encode is shown as an explicit placeholder. The widget never draws a
        // QR-like matrix it cannot back with real codewords.
        if self.encode_error.is_some() {
            context.fill_rect(rect, surface);
            context.draw_rect_stroke(rect, border_color, 1);

            let font = crate::widget::metrics::effective_font(style);
            let text = "QR encode error";
            let metrics = context.measure_text(text, font);
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
            let line = Rect::new(rect.x, text_y, rect.width, metrics.height);
            context.draw_text_fitted_cow(
                line,
                text,
                font,
                module_color,
                HorizontalAlignment::Center,
            );
            return;
        }

        let module = self.module_size.max(1);
        let size = self.matrix.size;
        let total_modules = size + self.quiet_zone * 2;
        let total_pixels = total_modules * module;

        // Center the symbol in the available geometry, then **clamp the origin once** and use
        // that single value for both the quiet-zone face and every module.
        //
        // The face was clamped (`offset_x.max(rect.x)`) while the module loop used the raw
        // `offset_x`, so a symbol larger than its control painted its modules outside the light
        // margin it had just drawn. The two must not be derived separately: the margin only
        // reads as a margin if the modules sit inside the same rectangle.
        let available_x = (rect.width.saturating_sub(total_pixels) / 2) as i32;
        let available_y = (rect.height.saturating_sub(total_pixels) / 2) as i32;
        let symbol_x = (rect.x + available_x).max(rect.x);
        let symbol_y = (rect.y + available_y).max(rect.y);
        let symbol_side = total_pixels.min(rect.width).min(rect.height);

        // Draw the quiet-zone surface for the entire symbol area.
        context.fill_rect(Rect::new(symbol_x, symbol_y, symbol_side, symbol_side), surface);

        // The logo cut-out: modules inside it are treated as light below, and the hole is filled
        // with the quiet-zone colour before the mark is painted, so the two always agree (MISC-03).
        let logo_hole = self.logo_hole();
        let in_logo_hole = |row: u32, col: u32| match logo_hole {
            Some((origin_row, origin_col, side)) => {
                row >= origin_row
                    && row < origin_row + side
                    && col >= origin_col
                    && col < origin_col + side
            }
            None => false,
        };

        for row in 0..size {
            // Horizontal run merging (K-P14): consecutive dark modules in one row become one
            // rectangle. A version-10 symbol is 57x57, so the per-module loop issued up to 3 249
            // fill commands a frame; a real payload is far closer to one run per row.
            let mut col = 0u32;
            while col < size {
                if !self.matrix.get(row, col) || in_logo_hole(row, col) {
                    col += 1;
                    continue;
                }
                let run_start = col;
                while col < size && self.matrix.get(row, col) && !in_logo_hole(row, col) {
                    col += 1;
                }
                let run_len = col - run_start;
                let x = symbol_x + (self.quiet_zone + run_start) as i32 * module as i32;
                let y = symbol_y + (self.quiet_zone + row) as i32 * module as i32;

                // Clamp to widget bounds.
                if y + module as i32 <= rect.y || y >= rect.y + rect.height as i32 {
                    continue;
                }
                if x + module as i32 <= rect.x || x >= rect.x + rect.width as i32 {
                    continue;
                }

                let run_px = run_len * module;
                let cell_w = run_px.min((rect.x + rect.width as i32 - x).max(0) as u32);
                let cell_h = module.min((rect.y + rect.height as i32 - y).max(0) as u32);
                if cell_w == 0 || cell_h == 0 {
                    continue;
                }
                context.fill_rect(Rect::new(x, y, cell_w, cell_h), module_color);
            }
        }

        // The cut-out itself, then the caller's mark on top of it. Painting the hole before the
        // mark means a transparent or failed icon still leaves a clean, scannable light square.
        if let Some((origin_row, origin_col, side)) = logo_hole {
            let hole = Rect::new(
                symbol_x + (self.quiet_zone + origin_col) as i32 * module as i32,
                symbol_y + (self.quiet_zone + origin_row) as i32 * module as i32,
                side * module,
                side * module,
            );
            context.fill_rect(hole, surface);
            if let Some(source) = self.logo {
                if let Err(error) =
                    crate::widget::paint_icon(context, source.into(), hole, module_color)
                {
                    log::warn!("QR logo could not be painted: {error:?}");
                }
            }
        }
    }
}

impl EventHandler for QRCode {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;

    #[test]
    fn qr_code_creation() {
        let qr = QRCode::new(Rect::new(0, 0, 120, 120));
        assert!(qr.data().is_empty());
        assert_eq!(qr.module_size(), 4);
        assert_eq!(qr.kind(), WidgetKind::QRCode);
    }

    #[test]
    fn qr_code_set_data() {
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        qr.set_data("Hello, World!");
        assert_eq!(qr.data(), "Hello, World!");
    }

    #[test]
    fn qr_code_set_module_size() {
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        assert_eq!(qr.module_size(), 4);
        qr.set_module_size(8);
        assert_eq!(qr.module_size(), 8);
        qr.set_module_size(0); // should clamp to 1
        assert_eq!(qr.module_size(), 1);
    }

    /// Encoding is a pure function of the payload and the EC level. Two encoders fed the same
    /// bytes must produce identical module grids.
    #[test]
    fn qr_encoding_is_deterministic() {
        let first = encode_byte_mode(b"test-data", QrEcLevel::L).expect("payload must encode");
        let second = encode_byte_mode(b"test-data", QrEcLevel::L).expect("payload must encode");
        assert_eq!(first, second, "same payload must produce the same matrix");

        let mut qr1 = QRCode::new(Rect::new(0, 0, 120, 120));
        qr1.set_data("test-data");
        let mut qr2 = QRCode::new(Rect::new(0, 0, 120, 120));
        qr2.set_data("test-data");
        assert_eq!(qr1.matrix, qr2.matrix);
        assert!(qr1.encode_error().is_none());
    }

    #[test]
    fn qr_code_different_data_different_output() {
        let first = encode_byte_mode(b"data-a", QrEcLevel::L).expect("payload must encode");
        let second = encode_byte_mode(b"data-b", QrEcLevel::L).expect("payload must encode");
        assert_ne!(
            first.modules, second.modules,
            "different data should produce different matrices"
        );
    }

    #[test]
    fn qr_code_svg_output() {
        let mut qr = QRCode::new(Rect::new(0, 0, 100, 100));
        qr.set_data("QR SVG Test");
        let svg = crate::widget::svg::render_to_svg(&mut qr);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
        // Should contain fill operations (white background + black modules).
        assert!(svg.contains("fill="));
    }

    #[test]
    fn qr_code_event_handler_delegates() {
        let mut qr = QRCode::new(Rect::new(0, 0, 100, 100));
        // Should not panic.
        qr.handle_event(&Event::MouseMove { pos: Point::new(10, 10) });
        qr.handle_event(&Event::KeyDown((65, 0)));
    }

    /// The payload fits version 1 at EC level L. This test asserts the structural invariants
    /// that only a real QR encoder satisfies: the three finder patterns, their light
    /// separators, both timing patterns, the dark module, and two agreeing copies of format
    /// information whose BCH remainder is zero and whose EC bits decode to L.
    #[test]
    fn hello_world_matches_standard_qr_structure() {
        let matrix =
            encode_byte_mode(b"HELLO WORLD", QrEcLevel::L).expect("HELLO WORLD must encode");
        assert_eq!(matrix.size, 21, "11 bytes fit version 1 at EC L");
        let size = matrix.size;

        let canonical_finder: Vec<bool> = (0..7)
            .flat_map(|row| (0..7).map(move |col| (row, col)))
            .map(|(row, col)| {
                let outer = row == 0 || row == 6 || col == 0 || col == 6;
                let inner = (2..=4).contains(&row) && (2..=4).contains(&col);
                outer || inner
            })
            .collect();
        let read_finder = |row0: u32, col0: u32| -> Vec<bool> {
            (0..7)
                .flat_map(|row| (0..7).map(move |col| (row, col)))
                .map(|(row, col)| matrix.get(row0 + row, col0 + col))
                .collect()
        };

        assert_eq!(read_finder(0, 0), canonical_finder, "top-left finder");
        assert_eq!(read_finder(0, size - 7), canonical_finder, "top-right finder");
        assert_eq!(read_finder(size - 7, 0), canonical_finder, "bottom-left finder");

        // Separators: the eighth row/column of each finder block is light.
        for index in 0..8 {
            assert!(!matrix.get(7, index), "top-left separator row");
            assert!(!matrix.get(index, 7), "top-left separator column");
            assert!(!matrix.get(7, size - 1 - index), "top-right separator row");
            assert!(!matrix.get(index, size - 8), "top-right separator column");
            assert!(!matrix.get(size - 8, index), "bottom-left separator row");
            assert!(!matrix.get(size - 1 - index, 7), "bottom-left separator column");
        }

        // Timing patterns alternate dark/light, starting dark at index 8.
        for index in 8..size - 8 {
            let expected = index % 2 == 0;
            assert_eq!(matrix.get(6, index), expected, "horizontal timing at {index}");
            assert_eq!(matrix.get(index, 6), expected, "vertical timing at {index}");
        }

        assert!(matrix.get(size - 8, 8), "the dark module must be dark");

        // Read both format-information copies. The bit positions are defined by the standard.
        let first_copy = |index: u32| -> bool {
            match index {
                0..=5 => matrix.get(index, 8),
                6 => matrix.get(7, 8),
                7 => matrix.get(8, 8),
                8 => matrix.get(8, 7),
                9..=14 => matrix.get(8, 14 - index),
                _ => unreachable!("format information has 15 bits"),
            }
        };
        let second_copy = |index: u32| -> bool {
            if index < 8 {
                matrix.get(8, size - 1 - index)
            } else {
                matrix.get(size - 15 + index, 8)
            }
        };

        let mut first = 0u16;
        let mut second = 0u16;
        for index in 0..15 {
            if first_copy(index) {
                first |= 1 << index;
            }
            if second_copy(index) {
                second |= 1 << index;
            }
        }
        assert_eq!(first, second, "both format-information copies must agree");

        // Independent BCH check: the unmasked 15-bit field must be divisible by 0x537.
        let unmasked = first ^ 0x5412;
        assert_eq!(
            bch_remainder(u32::from(unmasked), 0x537, 10),
            0,
            "format information must have a zero BCH remainder"
        );
        let data = unmasked >> 10;
        assert_eq!((data >> 3) & 0x03, 1, "EC bits must decode to level L");
        let mask = (data & 0x07) as u8;
        assert!(mask < 8, "decoded mask index must be in 0..=7");
    }

    /// Known BCH values from the QR specification. These pin the generators and the final
    /// `0x5412` format mask independently of the encoder's placement code.
    #[test]
    fn bch_information_known_values() {
        assert_eq!(format_bits(QrEcLevel::L, 0), 0x77C4);
        assert_eq!(format_bits(QrEcLevel::L, 1), 0x72F3);
        assert_eq!(format_bits(QrEcLevel::M, 0), 0x5412);
        assert_eq!(version_bits(7), 0x07C94);
        assert_eq!(version_bits(10), 0x0A4D3);
    }

    /// Every supported (version, EC level) pair must have a real block layout whose data and
    /// Reed-Solomon counts add up to the QR specification's total codewords for that version.
    #[test]
    fn block_layouts_cover_versions_one_to_ten() {
        let total_codewords = [26, 44, 70, 100, 134, 172, 196, 242, 292, 346];
        let l_data = [19, 34, 55, 80, 108, 136, 156, 194, 232, 274];
        let m_data = [16, 28, 44, 64, 86, 108, 124, 154, 182, 216];

        for version in 1u8..=10 {
            let index = (version - 1) as usize;
            for (ec, expected_data) in
                [(QrEcLevel::L, l_data[index]), (QrEcLevel::M, m_data[index])]
            {
                let spec =
                    block_spec(version, ec).expect("versions 1-10 must implement both EC levels");
                assert_eq!(spec.data_codewords, expected_data, "version {version} {ec:?}");

                let data = spec.short_blocks * spec.short_data_codewords
                    + (spec.num_blocks - spec.short_blocks) * (spec.short_data_codewords + 1);
                assert_eq!(data, spec.data_codewords);
                assert_eq!(
                    data + spec.num_blocks * spec.ec_per_block,
                    total_codewords[index],
                    "version {version} {ec:?} total codewords"
                );
            }
        }
    }

    /// Long division over GF(2) used as an independent format-information check.
    fn bch_remainder(value: u32, generator: u32, generator_degree: u32) -> u32 {
        if value == 0 {
            return 0;
        }
        let mut remainder = value;
        let mut degree = 31 - value.leading_zeros();
        while degree >= generator_degree {
            if (remainder >> degree) & 1 != 0 {
                remainder ^= generator << (degree - generator_degree);
            }
            if degree == 0 {
                break;
            }
            degree -= 1;
        }
        remainder
    }

    /// A payload beyond version 10-L's capacity must set `encode_error` and must not leave a
    /// fake matrix behind. Recovering with a valid payload clears the error.
    #[test]
    fn oversized_data_reports_encode_error_without_fake_matrix() {
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        let oversized = "a".repeat(3000);
        qr.set_data(&oversized);

        assert!(qr.encode_error().is_some(), "3000 bytes exceed version 40-L byte capacity");
        assert_eq!(qr.data(), oversized.as_str(), "data() still reports the requested payload");
        assert!(
            qr.matrix.modules.iter().all(|&dark| !dark),
            "a failed encode must not keep a fabricated matrix"
        );

        // The error branch must render the explicit placeholder rather than panicking.
        let svg = crate::widget::svg::render_to_svg(&mut qr);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));

        qr.set_data("ok");
        assert!(qr.encode_error().is_none(), "a valid payload clears the error");
        assert!(
            qr.matrix.modules.iter().any(|&dark| dark),
            "a successful encode produces a real matrix"
        );
    }

    /// The character-count indicator widens from 8 to 16 bits at version 10. The largest
    /// version 9-L payload is 230 bytes; the next byte must select version 10 (side 57).
    #[test]
    fn byte_mode_version_ten_length_boundary() {
        let version_nine =
            encode_byte_mode(&[b'A'; 230], QrEcLevel::L).expect("230 bytes fit version 9 at EC L");
        assert_eq!(version_nine.size, 53, "230 bytes fit version 9");

        let version_ten = encode_byte_mode(&[b'A'; 231], QrEcLevel::L)
            .expect("231 bytes need version 10 at EC L");
        assert_eq!(version_ten.size, 57, "231 bytes select version 10");

        // 256 bytes cannot be described by the 8-bit count indicator of versions 1-9; it must
        // use version 10's 16-bit field and still fit the version 10-L data capacity.
        let over_u8_count = encode_byte_mode(&[b'B'; 256], QrEcLevel::L)
            .expect("256 bytes fit version 10-L with a 16-bit count");
        assert_eq!(over_u8_count.size, 57, "256 bytes use the 16-bit count field");

        // The envelope now reaches the specification's version 40; only a payload beyond that
        // version's capacity is an error (MISC-04).
        let over_ten = encode_byte_mode(&[b'A'; 272], QrEcLevel::L)
            .expect("272 bytes now fit version 11 at EC L");
        assert_eq!(over_ten.size, matrix_size(11));
        let too_long = encode_byte_mode(&[b'A'; 3000], QrEcLevel::L);
        assert!(too_long.is_err(), "3000 bytes exceed version 40-L capacity");
    }

    /// The table-derived layouts for versions 1–10 must reproduce the explicit ones exactly, so
    /// extending the envelope to version 40 cannot silently change a symbol that used to fit.
    #[test]
    fn the_derived_block_tables_match_the_explicit_layouts() {
        for version in 1u8..=10 {
            for ec in [QrEcLevel::L, QrEcLevel::M, QrEcLevel::Q, QrEcLevel::H] {
                let explicit = block_spec(version, ec).expect("explicit layout");
                let derived = spec_from_tables(version, ec).expect("derived layout");
                assert_eq!(explicit, derived, "version {version} {ec:?}");
            }
        }
    }

    /// The highest version's capacities are the specification's published numbers.
    #[test]
    fn version_forty_holds_the_specifications_capacity() {
        assert_eq!(total_codewords(40), 3706);
        assert_eq!(block_spec(40, QrEcLevel::L).map(|spec| spec.data_codewords), Some(2956));
        assert_eq!(block_spec(40, QrEcLevel::H).map(|spec| spec.data_codewords), Some(1276));
        assert!(block_spec(41, QrEcLevel::L).is_none(), "the specification stops at 40");
    }

    /// A logo reserves a centred light square and recommends level H without changing the level.
    #[test]
    fn a_logo_carves_a_centred_hole_and_hints_a_higher_ec_level() {
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        qr.set_data("logo");
        assert!(qr.logo_hole().is_none(), "no logo means no hole");
        assert!(qr.logo_ec_hint().is_none());

        qr.set_logo(Some(IconSource::Builtin(crate::widget::IconName::Home)));
        let (row, col, side) = qr.logo_hole().expect("a logo carves a hole");
        assert_eq!(row, col, "the hole is centred");
        assert_eq!(row * 2 + side, qr.matrix.size, "the hole is centred on the symbol");
        assert!(qr.is_logo_module(row, col));
        assert!(!qr.is_logo_module(0, 0), "the finder patterns stay intact");
        assert_eq!(qr.logo_ec_hint(), Some(QrEcLevel::H));
        assert_eq!(qr.ec_level(), QrEcLevel::L, "the hint does not change the encoder");

        qr.set_logo(None);
        assert!(qr.logo_hole().is_none());
        assert!(qr.logo_ec_hint().is_none());
    }

    /// The logo is painted in a cut-out, so the rendered symbol differs from the plain one.
    #[test]
    fn a_logo_changes_the_painted_symbol() {
        let mut plain = QRCode::new(Rect::new(0, 0, 240, 240));
        plain.set_data("logo-paint");
        let without = crate::widget::svg::render_to_svg(&mut plain);

        let mut marked = QRCode::new(Rect::new(0, 0, 240, 240));
        marked.set_data("logo-paint");
        marked.set_logo(Some(IconSource::Builtin(crate::widget::IconName::Home)));
        let with = crate::widget::svg::render_to_svg(&mut marked);
        assert_ne!(without, with, "the hole and the mark must be visible");
    }

    /// All three finder patterns must be the **same shape**, with a light separator ring.
    #[test]
    fn all_three_finder_patterns_are_identical() {
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        qr.set_data("finder-shape");
        let matrix = &qr.matrix;

        let n = matrix.size as usize;
        let top_left: Vec<bool> = (0..7)
            .flat_map(|r| (0..7).map(move |c| (r, c)))
            .map(|(r, c)| matrix.get(r as u32, c as u32))
            .collect();
        let top_right: Vec<bool> = (0..7)
            .flat_map(|r| (0..7).map(move |c| (r, c)))
            .map(|(r, c)| matrix.get(r as u32, (n - 1 - c) as u32))
            .collect();
        let bottom_left: Vec<bool> = (0..7)
            .flat_map(|r| (0..7).map(move |c| (r, c)))
            .map(|(r, c)| matrix.get((n - 1 - r) as u32, c as u32))
            .collect();

        assert_eq!(top_left, top_right, "the top-right finder mirrors the top-left");
        assert_eq!(top_left, bottom_left, "the bottom-left finder mirrors the top-left");

        // And the canonical 7x7 shape: a dark ring, a light gap, a 3x3 dark core.
        let expected: Vec<bool> = (0..7)
            .flat_map(|r| (0..7).map(move |c| (r, c)))
            .map(|(r, c)| {
                let outer = r == 0 || r == 6 || c == 0 || c == 6;
                let inner = (2..=4).contains(&r) && (2..=4).contains(&c);
                outer || inner
            })
            .collect();
        assert_eq!(top_left, expected, "the finder is a ring plus a solid centre");
    }

    /// The separator ring between a finder pattern and the data area is **light**, so a
    /// decoder (and a reader) can tell where the 7x7 block ends.
    #[test]
    fn finder_patterns_are_ringed_by_a_light_separator() {
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        qr.set_data("separator");
        let matrix = &qr.matrix;
        let n = matrix.size as usize;

        for (r, c) in (0..8).flat_map(|r| (0..8).map(move |c| (r, c))) {
            if r == 7 || c == 7 {
                assert!(
                    !matrix.get(r as u32, c as u32),
                    "top-left separator at ({r},{c}) must be light"
                );
            }
        }
        for c in 0..8 {
            assert!(!matrix.get(7, c as u32));
            assert!(!matrix.get((n - 1 - c) as u32, 7), "bottom-left separator is light");
            assert!(!matrix.get(7, (n - 1 - c) as u32), "top-right separator is light");
        }
    }

    #[test]
    fn all_four_ec_levels_encode_and_the_property_round_trips() {
        for level in [QrEcLevel::L, QrEcLevel::M, QrEcLevel::Q, QrEcLevel::H] {
            let matrix = encode_byte_mode(b"EC-LEVEL", level)
                .unwrap_or_else(|err| panic!("{level:?} must encode: {err:?}"));
            assert!(matrix.size >= matrix_size(1) && matrix.size <= matrix_size(2));
            assert_eq!(decode_ec_level(&matrix), level);
        }
        let mut qr = QRCode::new(Rect::new(0, 0, 120, 120));
        qr.set_data("colours");
        qr.set_ec_level(QrEcLevel::H);
        assert_eq!(qr.ec_level(), QrEcLevel::H);
        qr.set_version_range(4, 6);
        assert_eq!(qr.version_range(), (4, 6));
        qr.set_foreground_color(Some(Color::rgb(10, 20, 30)));
        qr.set_background_color(Some(Color::rgb(240, 240, 240)));
        let svg = crate::widget::svg::render_to_svg(&mut qr);
        assert!(svg.starts_with("<svg"));
        assert_eq!(qr.get("ec_level"), Ok(CapabilityValue::String("H".to_string())));
    }

    /// A numeric payload uses the numeric segment and so picks a smaller version than the same
    /// number of bytes would (MISC-13).
    #[test]
    fn a_numeric_payload_takes_the_numeric_mode() {
        let digits = b"0123456789012345678901234567890123456789";
        let numeric =
            encode_segmented_in(digits, QrEcLevel::L, 1, 10).expect("40 digits must encode");
        let byte = encode_byte_mode_in(digits, QrEcLevel::L, 1, 10).expect("40 bytes must encode");
        assert!(
            numeric.size <= byte.size,
            "numeric mode cannot need a larger symbol (numeric {} vs byte {})",
            numeric.size,
            byte.size
        );
        assert!(numeric.size < byte.size, "40 digits are 2 versions smaller than 40 bytes");
    }

    /// An uppercase alphanumeric payload also takes the compact segment.
    #[test]
    fn an_alphanumeric_payload_takes_the_alnum_mode() {
        let text = b"HELLO WORLD 123";
        let alnum = encode_segmented_in(text, QrEcLevel::M, 1, 10).expect("alnum encodes");
        let byte = encode_byte_mode_in(text, QrEcLevel::M, 1, 10).expect("bytes encode");
        assert!(alnum.size <= byte.size);
    }

    /// A payload with nothing to compress takes the byte path, so its symbol is the one the
    /// byte-mode encoder has always produced.
    #[test]
    fn a_binary_payload_matches_the_byte_mode_encoder() {
        let data = b"lower case and \x01\x02 punctuation!";
        let segmented = encode_segmented_in(data, QrEcLevel::Q, 1, 10).expect("encodes");
        let byte = encode_byte_mode_in(data, QrEcLevel::Q, 1, 10).expect("encodes");
        assert_eq!(segmented.size, byte.size);
        assert_eq!(segmented.modules, byte.modules);
    }
}
