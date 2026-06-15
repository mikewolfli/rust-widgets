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
                    let tiff_data = &data[tiff_start..(tiff_start + seg_len - 6).min(data.len())];
                    parse_tiff_exif(tiff_data, &mut exif);
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
                pos += seg_len;
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

fn parse_tiff_exif(data: &[u8], exif: &mut ExifData) {
    if data.len() < 8 {
        return;
    }
    let little_endian = &data[0..4] == b"II\x2a\x00";

    let ifd_offset = if little_endian {
        u32::from_le_bytes([data[4], data[5], data[6], data[7]]) as usize
    } else {
        u32::from_be_bytes([data[4], data[5], data[6], data[7]]) as usize
    };

    if ifd_offset + 2 > data.len() {
        return;
    }

    let num_entries = if little_endian {
        u16::from_le_bytes([data[ifd_offset], data[ifd_offset + 1]])
    } else {
        u16::from_be_bytes([data[ifd_offset], data[ifd_offset + 1]])
    } as usize;

    for i in 0..num_entries {
        let entry_off = ifd_offset + 2 + i * 12;
        if entry_off + 12 > data.len() {
            break;
        }

        let tag = if little_endian {
            u16::from_le_bytes([data[entry_off], data[entry_off + 1]])
        } else {
            u16::from_be_bytes([data[entry_off], data[entry_off + 1]])
        };

        let read_u16 = |off: usize| -> u16 {
            if little_endian {
                u16::from_le_bytes([data[off], data[off + 1]])
            } else {
                u16::from_be_bytes([data[off], data[off + 1]])
            }
        };

        let read_u32 = |off: usize| -> u32 {
            if little_endian {
                u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
            } else {
                u32::from_be_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
            }
        };

        let read_rational = |off: usize| -> Option<f64> {
            if off + 8 <= data.len() {
                let num = read_u32(off) as f64;
                let den = read_u32(off + 4) as f64;
                if den != 0.0 {
                    Some(num / den)
                } else {
                    None
                }
            } else {
                None
            }
        };

        let value_offset = |entry_off: usize| -> usize {
            if little_endian {
                u32::from_le_bytes([
                    data[entry_off + 8],
                    data[entry_off + 9],
                    data[entry_off + 10],
                    data[entry_off + 11],
                ]) as usize
            } else {
                u32::from_be_bytes([
                    data[entry_off + 8],
                    data[entry_off + 9],
                    data[entry_off + 10],
                    data[entry_off + 11],
                ]) as usize
            }
        };

        match tag {
            256 => exif.exif_width = Some(read_u32(entry_off + 8)),
            257 => exif.exif_height = Some(read_u32(entry_off + 8)),
            271 => {
                // Make
                let off = value_offset(entry_off);
                if off + 32 <= data.len() {
                    let end = data[off..].iter().position(|&b| b == 0).unwrap_or(0).min(32);
                    exif.make = String::from_utf8_lossy(&data[off..off + end]).to_string();
                }
            }
            272 => {
                // Model
                let off = value_offset(entry_off);
                if off + 32 <= data.len() {
                    let end = data[off..].iter().position(|&b| b == 0).unwrap_or(0).min(32);
                    exif.model = String::from_utf8_lossy(&data[off..off + end]).to_string();
                }
            }
            274 => exif.orientation = Some(read_u16(entry_off + 8) as u8),
            306 => {
                // DateTime
                let off = value_offset(entry_off);
                if off + 20 <= data.len() {
                    let end = data[off..].iter().position(|&b| b == 0).unwrap_or(0).min(20);
                    exif.date_time =
                        Some(String::from_utf8_lossy(&data[off..off + end]).to_string());
                }
            }
            33437 => exif.iso = Some(read_u16(entry_off + 8) as u32),
            37386 => exif.focal_length = read_rational(entry_off + 8),
            33434 => exif.exposure_time = read_rational(entry_off + 8),
            37377 => {
                exif.aperture = {
                    let f = read_u32(entry_off + 8) as f64;
                    Some(f / 100.0)
                }
            }
            _ => {}
        }
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
}
