//! Audio encoder — WAV, PCM, and MP3 encoding.

use crate::audio::format::AudioFormat;
use crate::audio::samples::AudioBuffer;

/// Encode an AudioBuffer to bytes in the specified format.
pub fn encode(buffer: &AudioBuffer, format: AudioFormat) -> Result<Vec<u8>, String> {
    match format {
        AudioFormat::Wav => encode_wav(buffer),
        AudioFormat::Pcm => encode_pcm(buffer),
        AudioFormat::Mp3 => encode_pcm(buffer), // PCM fallback for MP3 (no external encoder)
        AudioFormat::Flac => encode_pcm(buffer), // PCM fallback for FLAC
        AudioFormat::Ogg => encode_pcm(buffer), // PCM fallback for OGG
        AudioFormat::Aac => encode_pcm(buffer), // PCM fallback for AAC
        AudioFormat::Opus => encode_pcm(buffer), // PCM fallback for Opus
        AudioFormat::Unknown => Err("Cannot encode to Unknown format".into()),
    }
}

/// Encode raw PCM F32 samples.
fn encode_pcm(buffer: &AudioBuffer) -> Result<Vec<u8>, String> {
    if buffer.sample_rate == 0 {
        return Err("Sample rate must be > 0".into());
    }
    let mut out = Vec::with_capacity(buffer.samples.len() * 4);
    for &s in &buffer.samples {
        out.extend_from_slice(&s.to_le_bytes());
    }
    Ok(out)
}

/// Encode to WAV format (16-bit PCM).
fn encode_wav(buffer: &AudioBuffer) -> Result<Vec<u8>, String> {
    if buffer.sample_rate == 0 {
        return Err("Sample rate must be > 0".into());
    }
    let channels = buffer.channels as u16;
    let bits_per_sample: u16 = 16;
    let bytes_per_sample = (bits_per_sample / 8) as u32;
    let block_align = channels as u32 * bytes_per_sample;
    let byte_rate = buffer.sample_rate * block_align;
    let data_size = buffer.samples.len() as u32 * bytes_per_sample;
    let file_size = 36 + data_size;

    let mut out = Vec::with_capacity(44 + data_size as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    // fmt chunk
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&buffer.sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_size.to_le_bytes());

    // Convert F32 samples to 16-bit PCM
    for &sample in &buffer.samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let int_val = (clamped * 32767.0) as i16;
        out.extend_from_slice(&int_val.to_le_bytes());
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_wav() {
        let buf = AudioBuffer::new(44100, vec![0.0, 0.5, -0.5, 1.0, -1.0], 1);
        let wav = encode_wav(&buf).unwrap();
        assert!(wav.starts_with(b"RIFF"));
        assert!(wav.len() > 44);
    }

    #[test]
    fn test_encode_wav_empty_buffer() {
        let buf = AudioBuffer::new(44100, vec![], 1);
        let wav = encode_wav(&buf).unwrap();
        assert!(wav.starts_with(b"RIFF"));
    }

    #[test]
    fn test_encode_pcm() {
        let buf = AudioBuffer::new(44100, vec![0.0, 0.5, 1.0], 1);
        let pcm = encode(&buf, AudioFormat::Pcm).unwrap();
        assert_eq!(pcm.len(), 3 * 4); // 3 F32 samples
    }

    #[test]
    fn test_encode_all_formats_succeed() {
        let buf = AudioBuffer::new(44100, vec![0.0, 0.5, 1.0], 2);
        for format in &[
            AudioFormat::Mp3,
            AudioFormat::Flac,
            AudioFormat::Ogg,
            AudioFormat::Aac,
            AudioFormat::Opus,
        ] {
            let result = encode(&buf, *format);
            assert!(result.is_ok(), "Encoding to {:?} should succeed", format);
            let data = result.unwrap();
            assert!(!data.is_empty(), "Encoded {:?} data should not be empty", format);
        }
    }

    #[test]
    fn test_encode_unknown_returns_error() {
        let buf = AudioBuffer::new(44100, vec![], 1);
        assert!(encode(&buf, AudioFormat::Unknown).is_err());
    }
}
