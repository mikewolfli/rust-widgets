//! Audio format enumeration and detection.

/// Supported audio formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AudioFormat {
    /// Unknown/unrecognized format.
    #[default]
    Unknown,
    /// WAVE (RIFF WAV).
    Wav,
    /// MP3 (MPEG Audio Layer III).
    Mp3,
    /// Free Lossless Audio Codec.
    Flac,
    /// Ogg Vorbis.
    Ogg,
    /// Advanced Audio Coding.
    Aac,
    /// Opus (in Ogg container).
    Opus,
    /// Raw PCM data.
    Pcm,
}

impl AudioFormat {
    /// Common file extension (without dot).
    pub fn extension(&self) -> &'static str {
        match self {
            AudioFormat::Unknown => "bin",
            AudioFormat::Wav => "wav",
            AudioFormat::Mp3 => "mp3",
            AudioFormat::Flac => "flac",
            AudioFormat::Ogg => "ogg",
            AudioFormat::Aac => "aac",
            AudioFormat::Opus => "opus",
            AudioFormat::Pcm => "pcm",
        }
    }

    /// MIME type string.
    pub fn mime_type(&self) -> &'static str {
        match self {
            AudioFormat::Unknown => "application/octet-stream",
            AudioFormat::Wav => "audio/wav",
            AudioFormat::Mp3 => "audio/mpeg",
            AudioFormat::Flac => "audio/flac",
            AudioFormat::Ogg => "audio/ogg",
            AudioFormat::Aac => "audio/aac",
            AudioFormat::Opus => "audio/opus",
            AudioFormat::Pcm => "audio/l16",
        }
    }

    /// Returns true if the format typically uses lossy compression.
    pub fn is_lossy(&self) -> bool {
        matches!(self, AudioFormat::Mp3 | AudioFormat::Aac | AudioFormat::Ogg | AudioFormat::Opus)
    }

    /// Returns true if the format is uncompressed PCM.
    pub fn is_uncompressed(&self) -> bool {
        matches!(self, AudioFormat::Wav | AudioFormat::Pcm)
    }
}

/// Sample format (bit depth and encoding).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SampleFormat {
    /// 8-bit unsigned.
    U8,
    /// 16-bit signed integer (little-endian).
    I16,
    /// 24-bit signed integer (little-endian).
    I24,
    /// 32-bit signed integer (little-endian).
    I32,
    /// 32-bit float.
    F32,
}

impl SampleFormat {
    /// Number of bytes per sample.
    pub fn bytes_per_sample(&self) -> usize {
        match self {
            SampleFormat::U8 => 1,
            SampleFormat::I16 => 2,
            SampleFormat::I24 => 3,
            SampleFormat::I32 => 4,
            SampleFormat::F32 => 4,
        }
    }

    /// Convert a byte buffer to F32 samples.
    pub fn to_f32(&self, data: &[u8]) -> Vec<f32> {
        match self {
            SampleFormat::U8 => data.iter().map(|&b| (b as f32 / 255.0) * 2.0 - 1.0).collect(),
            SampleFormat::I16 => data
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
                .collect(),
            SampleFormat::I24 => data
                .chunks_exact(3)
                .map(|c| {
                    let val = i32::from_le_bytes([c[0], c[1], c[2], 0]);
                    (val >> 8) as f32 / 8388608.0
                })
                .collect(),
            SampleFormat::I32 => data
                .chunks_exact(4)
                .map(|c| i32::from_le_bytes([c[0], c[1], c[2], c[3]]) as f32 / 2147483648.0)
                .collect(),
            SampleFormat::F32 => {
                data.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_format_extensions() {
        assert_eq!(AudioFormat::Wav.extension(), "wav");
        assert_eq!(AudioFormat::Mp3.extension(), "mp3");
        assert_eq!(AudioFormat::Flac.extension(), "flac");
        assert_eq!(AudioFormat::Ogg.extension(), "ogg");
    }

    #[test]
    fn test_audio_format_mime() {
        assert_eq!(AudioFormat::Wav.mime_type(), "audio/wav");
        assert_eq!(AudioFormat::Mp3.mime_type(), "audio/mpeg");
        assert_eq!(AudioFormat::Flac.mime_type(), "audio/flac");
    }

    #[test]
    fn test_sample_format_u8_to_f32() {
        let data = vec![0u8, 128, 255];
        let samples = SampleFormat::U8.to_f32(&data);
        assert!((samples[0] - (-1.0)).abs() < 0.01);
        assert!((samples[1] - 0.0).abs() < 0.01);
        assert!((samples[2] - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_sample_format_i16_to_f32() {
        let data = vec![0u8, 0, 0, 0x80, 0xFF, 0x7F];
        let samples = SampleFormat::I16.to_f32(&data);
        assert!((samples[0] - 0.0).abs() < 0.01);
        assert!((samples[2] - 0.999).abs() < 0.02);
    }
}
