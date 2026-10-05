// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
///
/// All header fields are 16- or 32-bit little-endian, so the sample rate, the
/// computed byte rate, block align and the total RIFF/data sizes must all fit
/// their fields before anything is written. The arithmetic is done in `u64` and
/// checked, so a pathological `sample_rate` (e.g. `u32::MAX`) or an enormous
/// buffer is rejected with an explicit error instead of silently wrapping the
/// `byte_rate`/`file_size` in the header and producing a file no decoder can
/// trust (N-S-52). Normal PCM/WAV output is byte-for-byte unchanged.
fn encode_wav(buffer: &AudioBuffer) -> Result<Vec<u8>, String> {
    if buffer.sample_rate == 0 {
        return Err("Sample rate must be > 0".into());
    }
    let channels = buffer.channels();
    if channels == 0 {
        return Err("Channel count must be > 0".into());
    }
    let bits_per_sample: u16 = 16;
    let bytes_per_sample: u64 = (bits_per_sample / 8) as u64;
    // `block_align` is a 16-bit field; compute in `u64` so a large channel count
    // cannot wrap it, then narrow after the check.
    let block_align = channels as u64 * bytes_per_sample;
    if block_align > u16::MAX as u64 {
        return Err(format!(
            "WAV block align {block_align} (channels {channels} × {bytes_per_sample} bytes) \
             exceeds the 16-bit header field"
        ));
    }
    // `byte_rate` and both sizes live in 32-bit fields; compute in `u64` and
    // reject anything that would not round-trip through those fields.
    let byte_rate = buffer.sample_rate as u64 * block_align;
    if byte_rate > u32::MAX as u64 {
        return Err(format!(
            "WAV byte rate {byte_rate} (sample_rate {} × block_align {block_align}) exceeds the \
             32-bit header field; the sample rate is too high for WAV output",
            buffer.sample_rate
        ));
    }
    // Each F32 sample becomes one 16-bit sample, so the count must itself be
    // representable before it is doubled.
    let sample_count = buffer.samples.len() as u64;
    let data_size = sample_count.checked_mul(bytes_per_sample).ok_or_else(|| {
        format!("WAV data size overflows: {sample_count} samples × {bytes_per_sample} bytes")
    })?;
    if data_size > u32::MAX as u64 {
        return Err(format!(
            "WAV data chunk of {data_size} bytes exceeds the 32-bit size field; the buffer is \
             too large for WAV output"
        ));
    }
    // RIFF size = 4 ("WAVE") + 8 + fmt chunk (16) + 8 + data chunk, i.e. 36 + data.
    let file_size = 36u64 + data_size;
    if file_size > u32::MAX as u64 {
        return Err(format!(
            "WAV RIFF size {file_size} exceeds the 32-bit header field; the buffer is too large \
             for WAV output"
        ));
    }
    let data_size = data_size as u32;
    let file_size = file_size as u32;
    let byte_rate = byte_rate as u32;
    let block_align = block_align as u16;
    // The channel-count field in the fmt chunk is 16-bit; widen the validated
    // `u8` count so it is written as the two bytes the format requires.
    let channels_field = channels as u16;

    let mut out = Vec::with_capacity(44 + data_size as usize);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&file_size.to_le_bytes());
    out.extend_from_slice(b"WAVE");

    // fmt chunk
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels_field.to_le_bytes());
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

    /// N-S-52: `sample_rate = u32::MAX` must be rejected explicitly rather than
    /// wrapping `byte_rate` in the header. 0xFFFFFFFF × 2 overflows `u32`.
    #[test]
    fn test_encode_wav_rejects_unrepresentable_byte_rate() {
        let buf = AudioBuffer::new(u32::MAX, vec![0.0, 0.0], 2);
        let err = encode_wav(&buf).unwrap_err();
        assert!(err.contains("byte rate"), "got: {err}");
    }

    /// A sample rate that fits `u32` but whose `byte_rate` just overflows is
    /// still rejected.
    #[test]
    fn test_encode_wav_rejects_byte_rate_boundary() {
        // byte_rate = sample_rate * block_align; pick the first value over the line.
        let sample_rate = u32::MAX / 2 + 1; // * 2 channels = just over u32::MAX
        let buf = AudioBuffer::new(sample_rate, vec![0.0, 0.0], 2);
        assert!(encode_wav(&buf).is_err());
    }

    /// A normal buffer must still produce a header whose sizes agree with the
    /// written bytes, i.e. the range checks left valid output untouched.
    #[test]
    fn test_encode_wav_header_is_consistent() {
        let buf = AudioBuffer::new(44100, vec![0.0, 0.5, -0.5, 1.0], 1);
        let wav = encode_wav(&buf).unwrap();
        // data chunk size field (offset 40) must equal samples * 2 bytes.
        let data_size = u32::from_le_bytes([wav[40], wav[41], wav[42], wav[43]]);
        assert_eq!(data_size, 4 * 2);
        // total length is header + data.
        assert_eq!(wav.len(), 44 + data_size as usize);
        // RIFF size field (offset 4) must be 36 + data_size.
        let riff_size = u32::from_le_bytes([wav[4], wav[5], wav[6], wav[7]]);
        assert_eq!(riff_size, 36 + data_size);
        // byte_rate field (offset 28) = sample_rate * block_align.
        let byte_rate = u32::from_le_bytes([wav[28], wav[29], wav[30], wav[31]]);
        assert_eq!(byte_rate, 44100 * 2);
    }
}
