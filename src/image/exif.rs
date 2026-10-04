// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! EXIF data extraction from JPEG and TIFF image files.

use crate::image::format::ExifData;

/// Extract EXIF metadata from image bytes (JPEG or TIFF).
/// Returns default ExifData if no EXIF is found.
pub fn extract_exif(data: &[u8]) -> ExifData {
    let mut exif = ExifData::default();

    // JPEG EXIF is in APP1 marker (0xFFE1)
    if data.len() > 4 && data[0] == 0xFF && data[1] == 0xD8 {
        // Parse JPEG segments for APP1 EXIF
        let mut pos = 2;
        while pos + 4 < data.len() {
            if data[pos] != 0xFF {
                pos += 1;
                continue;
            }
            let marker = data[pos + 1];
            let seg_len = if pos + 4 <= data.len() {
                u16::from_be_bytes([data[pos + 2], data[pos + 3]]) as usize
            } else {
                break;
            };

            if marker == 0xE1 && seg_len >= 8 {
                // APP1 - check for EXIF header "Exif\0\0"
                if pos + 10 < data.len() && &data[pos + 4..pos + 10] == b"Exif\x00\x00" {
                    // Parse TIFF structure inside APP1
                    let tiff_start = pos + 10;
                    let tiff_len = seg_len - 8;
                    if let Some(tiff_end) = tiff_start.checked_add(tiff_len) {
                        if tiff_end <= data.len() {
                            parse_tiff_exif(&data[tiff_start..tiff_end], &mut exif);
                        }
                    }
                    break;
                }
            }

            if marker == 0xDA || marker == 0xD9 {
                break; // SOS or EOI
            }
            if marker != 0xD0
                && marker != 0xD1
                && marker != 0xD2
                && marker != 0xD3
                && marker != 0xD4
                && marker != 0xD5
                && marker != 0xD6
                && marker != 0xD7
                && marker != 0xD8
            {
                pos += 2 + seg_len;
            } else {
                pos += 2;
            }
        }
    }

    // Check for standalone TIFF EXIF
    if data.len() > 8 && (&data[0..4] == b"II\x2a\x00" || &data[0..4] == b"MM\x00\x2a") {
        parse_tiff_exif(data, &mut exif);
    }

    exif
}

/// Reads a 16-bit integer at `off` in the file's byte order.
fn read_u16_at(data: &[u8], off: usize, little_endian: bool) -> Option<u16> {
    let bytes = data.get(off..off.checked_add(2)?)?;
    Some(if little_endian {
        u16::from_le_bytes([bytes[0], bytes[1]])
    } else {
        u16::from_be_bytes([bytes[0], bytes[1]])
    })
}

/// Reads a 32-bit integer at `off` in the file's byte order.
fn read_u32_at(data: &[u8], off: usize, little_endian: bool) -> Option<u32> {
    let bytes = data.get(off..off.checked_add(4)?)?;
    Some(if little_endian {
        u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    } else {
        u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
    })
}

/// The byte width of a TIFF field type, or `None` for a type this parser does not understand.
fn type_size(field_type: u16) -> Option<usize> {
    match field_type {
        1 | 2 | 7 => Some(1), // BYTE, ASCII, UNDEFINED
        3 => Some(2),         // SHORT
        4 | 9 => Some(4),     // LONG, SLONG
        5 | 10 => Some(8),    // RATIONAL, SRATIONAL
        _ => None,
    }
}

/// Resolves an IFD entry's value bytes: inline for values that fit in four bytes, otherwise at the
/// offset stored in the value field.
///
/// Returns `None` for an unknown type, a truncated file, or an offset/byte-count that overflows,
/// which lets the caller skip that one entry without dropping the rest of the file.
fn entry_value<'a>(
    data: &'a [u8],
    entry: &'a [u8],
    field_type: u16,
    count: u32,
    little_endian: bool,
) -> Option<&'a [u8]> {
    let size = type_size(field_type)?;
    let byte_len = (count as usize).checked_mul(size)?;
    let value_field = entry.get(8..12)?;
    if byte_len <= 4 {
        // Inline: the value occupies the 4-byte field, left-justified in a little-endian file and
        // right-justified in a big-endian one.
        let start = if little_endian { 0 } else { 4 - byte_len };
        value_field.get(start..start + byte_len)
    } else {
        let offset = read_u32_at(value_field, 0, little_endian)? as usize;
        let end = offset.checked_add(byte_len)?;
        data.get(offset..end)
    }
}

/// Calls `visit(tag, type, count, value_bytes)` for every entry of the IFD at `ifd_offset`.
///
/// Every offset and length is checked before it is used, so a truncated file or an out-of-range
/// offset aborts this IFD rather than reading past the buffer. The entry count is a `u16`, so the
/// loop is finite even for a hostile header.
fn for_each_ifd_entry(
    data: &[u8],
    ifd_offset: usize,
    little_endian: bool,
    visit: &mut dyn FnMut(u16, u16, u32, &[u8]),
) {
    let Some(count) = read_u16_at(data, ifd_offset, little_endian) else { return };
    for i in 0..count as usize {
        let entry_off = match ifd_offset
            .checked_add(2)
            .and_then(|base| i.checked_mul(12).and_then(|step| base.checked_add(step)))
        {
            Some(off) => off,
            None => return,
        };
        let entry = match data.get(entry_off..entry_off.saturating_add(12)) {
            Some(slice) if slice.len() == 12 => slice,
            _ => return,
        };
        let tag = match read_u16_at(entry, 0, little_endian) {
            Some(t) => t,
            None => return,
        };
        let field_type = match read_u16_at(entry, 2, little_endian) {
            Some(t) => t,
            None => return,
        };
        let count = match read_u32_at(entry, 4, little_endian) {
            Some(c) => c,
            None => return,
        };
        if let Some(value) = entry_value(data, entry, field_type, count, little_endian) {
            visit(tag, field_type, count, value);
        }
    }
}

/// A NUL-terminated ASCII string, bounded by the value's `count` bytes.
fn read_ascii(value: &[u8]) -> String {
    let end = value.iter().position(|&b| b == 0).unwrap_or(value.len());
    String::from_utf8_lossy(&value[..end]).to_string()
}

fn value_u16(value: &[u8], little_endian: bool) -> Option<u16> {
    read_u16_at(value, 0, little_endian)
}

fn value_u32(value: &[u8], little_endian: bool) -> Option<u32> {
    read_u32_at(value, 0, little_endian)
}

fn value_rational(value: &[u8], little_endian: bool) -> Option<f64> {
    let numerator = read_u32_at(value, 0, little_endian)?;
    let denominator = read_u32_at(value, 4, little_endian)?;
    if denominator == 0 {
        None
    } else {
        Some(numerator as f64 / denominator as f64)
    }
}

/// Applies a recognised tag's value to the matching [`ExifData`] field.
///
/// One function serves both IFD0 and the Exif sub-IFD, because a few tools write tags like
/// `FocalLength` directly in IFD0 while the standard places them in the sub-IFD; accepting both
/// locations keeps a claimed-supported field from being silently dropped.
fn apply_known_tag(tag: u16, value: &[u8], little_endian: bool, exif: &mut ExifData) {
    match tag {
        256 => exif.exif_width = value_u32(value, little_endian),
        257 => exif.exif_height = value_u32(value, little_endian),
        271 => exif.make = read_ascii(value),
        272 => exif.model = read_ascii(value),
        274 => exif.orientation = value_u16(value, little_endian).map(|v| v as u8),
        306 => exif.date_time = Some(read_ascii(value)),
        33434 => exif.exposure_time = value_rational(value, little_endian),
        33437 => exif.aperture = value_rational(value, little_endian),
        34855 => exif.iso = value_u16(value, little_endian).map(|v| v as u32),
        37386 => exif.focal_length = value_rational(value, little_endian),
        36867 | 36868 => {
            if exif.date_time.is_none() {
                exif.date_time = Some(read_ascii(value));
            }
        }
        _ => {}
    }
}

/// Parses a GPS sub-IFD, decoding latitude and longitude into decimal degrees.
fn parse_gps_ifd(data: &[u8], ifd_offset: usize, little_endian: bool, exif: &mut ExifData) {
    let mut lat_ref: Option<String> = None;
    let mut lon_ref: Option<String> = None;
    let mut lat: Option<f64> = None;
    let mut lon: Option<f64> = None;

    for_each_ifd_entry(data, ifd_offset, little_endian, &mut |tag, _t, _c, value| match tag {
        1 => lat_ref = Some(read_ascii(value)),
        3 => lon_ref = Some(read_ascii(value)),
        2 => lat = gps_coordinate(value, little_endian),
        4 => lon = gps_coordinate(value, little_endian),
        _ => {}
    });

    exif.gps_latitude =
        lat.map(|degrees| if lat_ref.as_deref() == Some("S") { -degrees } else { degrees });
    exif.gps_longitude =
        lon.map(|degrees| if lon_ref.as_deref() == Some("W") { -degrees } else { degrees });
}

/// Decodes a GPS coordinate: three RATIONALs for degrees, minutes and seconds.
fn gps_coordinate(value: &[u8], little_endian: bool) -> Option<f64> {
    if value.len() < 24 {
        return None;
    }
    let degrees = value_rational(value, little_endian)?;
    let minutes = value_rational(&value[8..16], little_endian)?;
    let seconds = value_rational(&value[16..24], little_endian)?;
    Some(degrees + minutes / 60.0 + seconds / 3600.0)
}

fn parse_tiff_exif(data: &[u8], exif: &mut ExifData) {
    if data.len() < 8 {
        return;
    }
    let little_endian = &data[0..4] == b"II\x2a\x00";
    let big_endian = &data[0..4] == b"MM\x00\x2a";
    if !little_endian && !big_endian {
        return;
    }
    let Some(ifd0) = read_u32_at(data, 4, little_endian) else { return };
    let ifd0 = ifd0 as usize;

    // The two sub-IFD pointers live in IFD0 and are collected here so the sub-IFDs can be parsed
    // afterwards: the pointers only make sense in IFD0, and the sub-IFDs use their own tags.
    let mut exif_ifd: Option<usize> = None;
    let mut gps_ifd: Option<usize> = None;

    for_each_ifd_entry(
        data,
        ifd0,
        little_endian,
        &mut |tag, _field_type, _count, value| match tag {
            34665 => exif_ifd = value_u32(value, little_endian).map(|v| v as usize),
            34853 => gps_ifd = value_u32(value, little_endian).map(|v| v as usize),
            _ => apply_known_tag(tag, value, little_endian, exif),
        },
    );

    // Traverse the Exif and GPS sub-IFDs. A pointer back to IFD0 itself is refused, and each
    // sub-IFD is visited at most once, so a hostile file cannot make the parser loop.
    if let Some(offset) = exif_ifd.filter(|&o| o != ifd0) {
        for_each_ifd_entry(data, offset, little_endian, &mut |tag, _t, _c, value| {
            apply_known_tag(tag, value, little_endian, exif);
        });
    }
    if let Some(offset) = gps_ifd.filter(|&o| o != ifd0) {
        parse_gps_ifd(data, offset, little_endian, exif);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_exif_empty() {
        let exif = extract_exif(b"");
        assert_eq!(exif.make, "");
        assert_eq!(exif.model, "");
    }

    #[test]
    fn test_extract_exif_no_exif() {
        let data = b"\xFF\xD8\xFF\xE0\x00\x10JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00\xFF\xD9";
        let exif = extract_exif(data);
        assert_eq!(exif.make, "");
    }

    #[test]
    fn test_extract_exif_from_jpeg_with_app1() {
        // Minimal JPEG with APP1 EXIF
        let mut jpeg = vec![0xFF, 0xD8];
        // APP1 marker
        jpeg.push(0xFF);
        jpeg.push(0xE1);
        // APP1 segment length (including length field)
        let app1_data_len = 8 + 8 + 8; // Exif\0\0 + TIFF header + IFD
        let seg_len: u16 = 2 + app1_data_len as u16;
        jpeg.extend_from_slice(&seg_len.to_be_bytes());
        // Exif header
        jpeg.extend_from_slice(b"Exif\x00\x00");
        // Minimal TIFF (little-endian)
        jpeg.extend_from_slice(b"II\x2a\x00"); // TIFF header
        jpeg.extend_from_slice(&8u32.to_le_bytes()); // IFD offset = 8
        jpeg.extend_from_slice(&0u16.to_le_bytes()); // 0 entries
        jpeg.extend_from_slice(&[0u8; 4]); // next IFD offset = 0
                                           // EOI
        jpeg.extend_from_slice(&[0xFF, 0xD9]);

        let exif = extract_exif(&jpeg);
        // Should not crash, should return empty or parsed
        assert!(exif.make.is_empty());
    }

    #[test]
    fn test_extract_exif_string_without_nul_uses_bounded_field() {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II\x2a\x00");
        tiff.extend_from_slice(&8u32.to_le_bytes());
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&271u16.to_le_bytes());
        tiff.extend_from_slice(&2u16.to_le_bytes());
        tiff.extend_from_slice(&32u32.to_le_bytes());
        tiff.extend_from_slice(&26u32.to_le_bytes());
        tiff.extend_from_slice(&[0u8; 4]);
        tiff.extend_from_slice(b"AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");

        let exif = extract_exif(&tiff);
        assert_eq!(exif.make, "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
    }

    #[test]
    fn test_extract_exif_rejects_truncated_app1_without_panicking() {
        let jpeg = b"\xFF\xD8\xFF\xE1\x00\x20Exif\x00\x00II\x2a\x00\x08\x00";
        let exif = extract_exif(jpeg);
        assert!(exif.make.is_empty());
        assert!(exif.model.is_empty());
    }

    /// Builds a little-endian TIFF with the given IFD0 entries. Each entry is
    /// `(tag, type, count, value)`: values of four bytes or fewer are stored inline, and larger
    /// values are appended after the IFD with the entry's offset field pointing at them.
    fn build_tiff_le(entries: &[(u16, u16, u32, Vec<u8>)]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"II\x2a\x00");
        out.extend_from_slice(&8u32.to_le_bytes()); // IFD0 starts at offset 8
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());

        let mut data_offset = 8 + 2 + entries.len() * 12 + 4;
        for (tag, typ, count, value) in entries {
            out.extend_from_slice(&tag.to_le_bytes());
            out.extend_from_slice(&typ.to_le_bytes());
            out.extend_from_slice(&count.to_le_bytes());
            if value.len() <= 4 {
                let mut field = [0u8; 4];
                field[..value.len()].copy_from_slice(value);
                out.extend_from_slice(&field);
            } else {
                out.extend_from_slice(&(data_offset as u32).to_le_bytes());
                data_offset += value.len();
            }
        }
        out.extend_from_slice(&0u32.to_le_bytes()); // next IFD = 0
        for (_tag, _typ, _count, value) in entries {
            if value.len() > 4 {
                out.extend_from_slice(value);
            }
        }
        out
    }

    /// A RATIONAL value must be read from the offset its entry points to, not from the entry's own
    /// value field.
    #[test]
    fn exif_reads_rational_from_the_offset_it_points_to() {
        // 37386 (FocalLength) = 50/1, stored out of line.
        let rational = [50u32.to_le_bytes(), 1u32.to_le_bytes()].concat();
        let tiff = build_tiff_le(&[(37386, 5, 1, rational)]);
        let exif = extract_exif(&tiff);
        assert_eq!(exif.focal_length, Some(50.0));
    }

    /// ASCII values honour `count` and are read inline when they fit in four bytes.
    #[test]
    fn exif_reads_ascii_by_count_and_inline_values() {
        // Make: count 6, stored out of line as "Canon\0". Model: count 4, stored inline "ABC\0".
        let tiff =
            build_tiff_le(&[(271, 2, 6, b"Canon\0".to_vec()), (272, 2, 4, b"ABC\0".to_vec())]);
        let exif = extract_exif(&tiff);
        assert_eq!(exif.make, "Canon");
        assert_eq!(exif.model, "ABC");
    }

    /// The ExifIFD pointer (34665) must be traversed so sub-IFD fields like ExposureTime decode.
    #[test]
    fn exif_traverses_the_exif_sub_ifd() {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"II\x2a\x00");
        tiff.extend_from_slice(&8u32.to_le_bytes()); // IFD0 at 8
        tiff.extend_from_slice(&1u16.to_le_bytes()); // one entry
                                                     // IFD0 entry: tag 34665, type LONG (4), count 1, inline value = sub-IFD offset 26.
        tiff.extend_from_slice(&34665u16.to_le_bytes());
        tiff.extend_from_slice(&4u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&26u32.to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes()); // next IFD = 0

        // Sub-IFD at 26: one entry, ExposureTime (33434) = 1/125 out of line at 44.
        tiff.extend_from_slice(&1u16.to_le_bytes());
        tiff.extend_from_slice(&33434u16.to_le_bytes());
        tiff.extend_from_slice(&5u16.to_le_bytes());
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&44u32.to_le_bytes());
        tiff.extend_from_slice(&0u32.to_le_bytes()); // next IFD = 0

        // Rational at 44: 1 / 125.
        tiff.extend_from_slice(&1u32.to_le_bytes());
        tiff.extend_from_slice(&125u32.to_le_bytes());

        let exif = extract_exif(&tiff);
        assert_eq!(exif.exposure_time, Some(1.0 / 125.0));
    }

    /// A bad type and an out-of-range offset must be skipped, not panic, and not fabricate a value.
    #[test]
    fn exif_skips_bad_type_and_truncated_offsets() {
        // Type 0 is not a TIFF field type, and this offset points past the end of the file.
        let bad_type = build_tiff_le(&[(271, 0, 4, vec![0; 4])]);
        assert_eq!(extract_exif(&bad_type).make, "");

        let truncated = build_tiff_le(&[(271, 2, 40, vec![b'A'; 40])]);
        // The value field points beyond the file: the 40 bytes are not appended, so the offset is
        // out of range and `make` stays empty rather than panicking.
        let mut cut = truncated;
        cut.truncate(cut.len() - 40);
        assert_eq!(extract_exif(&cut).make, "");
    }

    /// Big-endian TIFF must read SHORT/LONG/RATIONAL values in big-endian order.
    #[test]
    fn exif_big_endian_values_are_read_correctly() {
        let mut tiff = Vec::new();
        tiff.extend_from_slice(b"MM\x00\x2a");
        tiff.extend_from_slice(&8u32.to_be_bytes()); // IFD0 at 8
        tiff.extend_from_slice(&3u16.to_be_bytes()); // three entries

        // Orientation (274), SHORT, count 1, inline value 6.
        tiff.extend_from_slice(&274u16.to_be_bytes());
        tiff.extend_from_slice(&3u16.to_be_bytes());
        tiff.extend_from_slice(&1u32.to_be_bytes());
        tiff.extend_from_slice(&[0, 0, 0, 6]); // right-justified inline SHORT

        // ImageWidth (256), LONG, count 1, inline value 1024.
        tiff.extend_from_slice(&256u16.to_be_bytes());
        tiff.extend_from_slice(&4u16.to_be_bytes());
        tiff.extend_from_slice(&1u32.to_be_bytes());
        tiff.extend_from_slice(&1024u32.to_be_bytes());

        // FocalLength (37386), RATIONAL, count 1, out of line at 8 + 2 + 36 + 4 = 50.
        tiff.extend_from_slice(&37386u16.to_be_bytes());
        tiff.extend_from_slice(&5u16.to_be_bytes());
        tiff.extend_from_slice(&1u32.to_be_bytes());
        tiff.extend_from_slice(&50u32.to_be_bytes());
        tiff.extend_from_slice(&0u32.to_be_bytes()); // next IFD = 0
        tiff.extend_from_slice(&2u32.to_be_bytes()); // rational numerator 2
        tiff.extend_from_slice(&3u32.to_be_bytes()); // rational denominator 3

        let exif = extract_exif(&tiff);
        assert_eq!(exif.orientation, Some(6));
        assert_eq!(exif.exif_width, Some(1024));
        assert_eq!(exif.focal_length, Some(2.0 / 3.0));
    }
}
