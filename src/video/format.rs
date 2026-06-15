//! Video container format detection and enumeration.

/// Supported video container formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ContainerFormat {
    /// Unknown/unrecognized format.
    Unknown,
    /// MP4 (ISO Base Media File Format).
    Mp4,
    /// Audio Video Interleave.
    Avi,
    /// QuickTime Movie.
    Mov,
    /// Matroska Video.
    Mkv,
    /// WebM (Matroska subset for Web).
    WebM,
    /// Flash Video.
    Flv,
    /// Windows Media Video.
    Wmv,
    /// Motion JPEG (raw MJPEG stream).
    Mjpeg,
}

impl ContainerFormat {
    /// Common file extension (without dot).
    pub fn extension(&self) -> &'static str {
        match self {
            ContainerFormat::Unknown => "bin",
            ContainerFormat::Mp4 => "mp4",
            ContainerFormat::Avi => "avi",
            ContainerFormat::Mov => "mov",
            ContainerFormat::Mkv => "mkv",
            ContainerFormat::WebM => "webm",
            ContainerFormat::Flv => "flv",
            ContainerFormat::Wmv => "wmv",
            ContainerFormat::Mjpeg => "mjpeg",
        }
    }

    /// MIME type string.
    pub fn mime_type(&self) -> &'static str {
        match self {
            ContainerFormat::Unknown => "application/octet-stream",
            ContainerFormat::Mp4 => "video/mp4",
            ContainerFormat::Avi => "video/x-msvideo",
            ContainerFormat::Mov => "video/quicktime",
            ContainerFormat::Mkv => "video/x-matroska",
            ContainerFormat::WebM => "video/webm",
            ContainerFormat::Flv => "video/x-flv",
            ContainerFormat::Wmv => "video/x-ms-wmv",
            ContainerFormat::Mjpeg => "video/x-motion-jpeg",
        }
    }
}

impl Default for ContainerFormat {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Detect video container format from magic bytes.
pub fn detect_container_format(data: &[u8]) -> ContainerFormat {
    if data.is_empty() {
        return ContainerFormat::Unknown;
    }
    // MJPEG: starts with JPEG SOI marker 0xFF 0xD8 — check early before 8-byte guard
    if data.len() >= 2 && data[0] == 0xFF && data[1] == 0xD8 {
        return ContainerFormat::Mjpeg;
    }
    // FLV: FLV header (only 3 bytes)
    if data.len() >= 3 && data[0] == b'F' && data[1] == b'L' && data[2] == b'V' {
        return ContainerFormat::Flv;
    }
    if data.len() < 8 {
        return ContainerFormat::Unknown;
    }
    // MP4: ftyp box
    if data.len() >= 8 && &data[4..8] == b"ftyp" {
        return ContainerFormat::Mp4;
    }
    // AVI: RIFF AVI
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"AVI " {
        return ContainerFormat::Avi;
    }
    // MOV: ftyp box (same as MP4) or moov
    if data.len() >= 8 && &data[4..8] == b"moov" {
        return ContainerFormat::Mov;
    }
    // MKV/WebM: EBML header
    if data.len() >= 4 && data[0] == 0x1A && data[1] == 0x45 && data[2] == 0xDF && data[3] == 0xA3 {
        // Check for WebM-specific DocType string "webm" in the EBML header
        if data.len() > 48 && data.windows(4).skip(8).any(|w| w == b"webm") {
            return ContainerFormat::WebM;
        }
        return ContainerFormat::Mkv;
    }
    // WMV: ASF header
    if data.len() >= 16 && &data[0..16] == b"0&\xb2u\x8ef\xcf\x11\xa6\xd9\x00\xaa\x00b\xcel" {
        return ContainerFormat::Wmv;
    }
    ContainerFormat::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_mp4() {
        let data = b"\x00\x00\x00\x20ftypmp42\x00\x00\x00\x00";
        assert_eq!(detect_container_format(data), ContainerFormat::Mp4);
    }

    #[test]
    fn test_detect_avi() {
        let mut data = b"RIFF".to_vec();
        data.extend_from_slice(&[0u8; 4]);
        data.extend_from_slice(b"AVI ");
        assert_eq!(detect_container_format(&data), ContainerFormat::Avi);
    }

    #[test]
    fn test_detect_flv() {
        assert_eq!(detect_container_format(b"FLV\x01"), ContainerFormat::Flv);
    }

    #[test]
    fn test_detect_mkv() {
        let data = b"\x1A\x45\xDF\xA3\x01\x00\x00\x00";
        assert_eq!(detect_container_format(data), ContainerFormat::Mkv);
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_container_format(b"not video"), ContainerFormat::Unknown);
    }

    #[test]
    fn test_detect_mjpeg() {
        assert_eq!(detect_container_format(&[0xFF, 0xD8, 0xFF, 0xE0]), ContainerFormat::Mjpeg);
    }

    #[test]
    fn test_format_extensions() {
        assert_eq!(ContainerFormat::Mp4.extension(), "mp4");
        assert_eq!(ContainerFormat::Mkv.extension(), "mkv");
        assert_eq!(ContainerFormat::WebM.extension(), "webm");
        assert_eq!(ContainerFormat::Flv.extension(), "flv");
        assert_eq!(ContainerFormat::Mjpeg.extension(), "mjpeg");
    }
}
