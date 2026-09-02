//! Audio encoder — WAV and raw PCM are encoded natively. Compressed formats
//! (MP3, FLAC, OGG, AAC, Opus) are encoded through FFmpeg and therefore
//! require the `video-codecs` feature; without it, `encode` returns an error
//! instead of writing raw PCM bytes under a compressed-format name.

use crate::audio::format::AudioFormat;
use crate::audio::samples::AudioBuffer;

#[cfg(feature = "video-codecs")]
use super::ffmpeg_encode;

/// Encode an AudioBuffer to bytes in the specified format.
///
/// WAV and raw PCM are always supported. Compressed formats (Mp3, Flac, Ogg,
/// Aac, Opus) are encoded through FFmpeg and require the `video-codecs`
/// feature; without it they return an explicit error.
pub fn encode(buffer: &AudioBuffer, format: AudioFormat) -> Result<Vec<u8>, String> {
    match format {
        AudioFormat::Wav => encode_wav(buffer),
        AudioFormat::Pcm => encode_pcm(buffer),
        // ── Real compressed encoding via FFmpeg when the feature is enabled ──
        #[cfg(feature = "video-codecs")]
        AudioFormat::Mp3
        | AudioFormat::Flac
        | AudioFormat::Ogg
        | AudioFormat::Aac
        | AudioFormat::Opus => ffmpeg_encode(buffer, format),
        // ── No FFmpeg: refuse instead of emitting bare PCM masquerading as a
        //    compressed format ──
        #[cfg(not(feature = "video-codecs"))]
        AudioFormat::Mp3
        | AudioFormat::Flac
        | AudioFormat::Ogg
        | AudioFormat::Aac
        | AudioFormat::Opus => {
            Err(format!("encoding {:?} requires the `video-codecs` feature (FFmpeg)", format))
        }
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
    #[cfg(feature = "video-codecs")]
    fn test_encode_all_formats_succeed() {
        // Use enough samples so encoders with minimum frame-size (e.g. FLAC)
        // have data to work with: ~0.37 seconds of 44100 Hz mono = 16384 samples.
        let samples: Vec<f32> = (0..16384)
            .map(|i| {
                let phase = (i as f32 / 44100.0 * 440.0 * 2.0 * std::f32::consts::PI).sin();
                phase * 0.5
            })
            .collect();
        let buf = AudioBuffer::new(44100, samples, 2);
        for format in &[
            AudioFormat::Mp3,
            AudioFormat::Flac,
            AudioFormat::Ogg,
            AudioFormat::Aac,
            AudioFormat::Opus,
        ] {
            let result = encode(&buf, *format);
            assert!(result.is_ok(), "Encoding to {:?} should succeed: {:?}", format, result);
            let data = result.unwrap();
            assert!(!data.is_empty(), "Encoded {:?} data should not be empty", format);
        }
    }

    #[test]
    #[cfg(not(feature = "video-codecs"))]
    fn test_encode_compressed_formats_report_missing_feature() {
        // Without `video-codecs` there is no real encoder for compressed
        // formats, so `encode` must fail loudly instead of returning bare PCM
        // bytes under a compressed-format name.
        let samples: Vec<f32> = (0..16384)
            .map(|i| {
                let phase = (i as f32 / 44100.0 * 440.0 * 2.0 * std::f32::consts::PI).sin();
                phase * 0.5
            })
            .collect();
        let buf = AudioBuffer::new(44100, samples, 2);
        for format in &[
            AudioFormat::Mp3,
            AudioFormat::Flac,
            AudioFormat::Ogg,
            AudioFormat::Aac,
            AudioFormat::Opus,
        ] {
            let err = encode(&buf, *format)
                .expect_err("encoding a compressed format without `video-codecs` must fail");
            assert!(
                err.contains("video-codecs"),
                "error for {:?} should name the `video-codecs` feature, got: {err}",
                format
            );
        }
    }

    #[test]
    fn test_encode_unknown_returns_error() {
        let buf = AudioBuffer::new(44100, vec![], 1);
        assert!(encode(&buf, AudioFormat::Unknown).is_err());
    }
}
