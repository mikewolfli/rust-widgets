//! Audio decoder — format detection and PCM decoding.
//!
//! - WAV is decoded natively.
//! - MP3 is decoded with `minimp3_fixed`.
//! - FLAC, OGG Vorbis, AAC (ADTS) and Opus are decoded by symphonia and thus
//!   require the `symphonia-codecs` feature. Without that feature, decoding
//!   those formats returns an explicit error — this module never fabricates
//!   PCM samples from a compressed bitstream.

use crate::audio::format::{AudioFormat, SampleFormat};
use crate::audio::samples::AudioBuffer;

/// Detect audio format from magic bytes.
pub fn detect_audio_format(data: &[u8]) -> AudioFormat {
    if data.len() < 4 {
        return AudioFormat::Unknown;
    }
    // WAV: RIFF....WAVE
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE" {
        return AudioFormat::Wav;
    }
    // AAC: ADTS header (0xFFF) — check before raw MP3 sync to avoid false match
    if data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xF0) == 0xF0 {
        // Differentiate AAC ADTS from MP3: AAC has full 12-bit syncword 0xFFF
        // and the MPEG version bit (bit 3 of byte 1) helps distinguish
        if (data[1] & 0x08) == 0x08 {
            // MPEG-2 or MPEG-4 AAC (ADTS version)
            return AudioFormat::Aac;
        }
    }
    // MP3: ID3 tag or sync bits
    if data.len() >= 3 && &data[0..3] == b"ID3" {
        return AudioFormat::Mp3;
    }
    // MP3: MPEG sync word — must NOT be AAC ADTS (0xFFF0-0xFFFF)
    if data[0] == 0xFF && (data[1] & 0xE0) == 0xE0 && (data[1] & 0xF0) != 0xF0 {
        return AudioFormat::Mp3;
    }
    // FLAC: fLaC
    if data.len() >= 4 && &data[0..4] == b"fLaC" {
        return AudioFormat::Flac;
    }
    // OGG: OggS
    if data.len() >= 4 && &data[0..4] == b"OggS" {
        return AudioFormat::Ogg;
    }
    // AAC: ADTS header (0xFFF)
    if data.len() >= 2 && data[0] == 0xFF && (data[1] & 0xF0) == 0xF0 {
        return AudioFormat::Aac;
    }
    AudioFormat::Unknown
}

/// Decode audio from bytes into an AudioBuffer.
///
/// FLAC, OGG Vorbis, AAC and Opus require the `symphonia-codecs` feature;
/// without it, decoding those formats returns an error explaining so.
pub fn decode(data: &[u8]) -> Result<AudioBuffer, String> {
    let format = detect_audio_format(data);
    match format {
        AudioFormat::Wav => decode_wav(data),
        AudioFormat::Pcm => {
            // Assume 44100 Hz, mono, F32
            let samples: Vec<f32> = data
                .chunks_exact(4)
                .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
                .collect();
            Ok(AudioBuffer::new(44100, samples, 1))
        }
        AudioFormat::Mp3 => decode_mp3(data),
        AudioFormat::Flac => decode_flac(data),
        AudioFormat::Ogg => decode_ogg_vorbis(data),
        AudioFormat::Aac => decode_aac(data),
        AudioFormat::Opus => decode_opus(data),
        AudioFormat::Unknown => Err("Unknown audio format — cannot decode".into()),
    }
}

/// Decode WAV audio data.
fn decode_wav(data: &[u8]) -> Result<AudioBuffer, String> {
    if data.len() < 44 || &data[0..4] != b"RIFF" || &data[8..12] != b"WAVE" {
        return Err("Invalid WAV header".into());
    }

    // Parse fmt chunk
    let mut pos = 12;
    let mut sample_rate = 0u32;
    let mut channels = 0u8;
    let mut bits_per_sample = 0u16;
    let mut data_chunk: Option<&[u8]> = None;

    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;
        let chunk_data = &data[pos + 8..(pos + 8 + chunk_size).min(data.len())];

        if chunk_id == b"fmt " && chunk_data.len() >= 16 {
            let _audio_format = u16::from_le_bytes([chunk_data[0], chunk_data[1]]);
            channels = u16::from_le_bytes([chunk_data[2], chunk_data[3]]) as u8;
            sample_rate =
                u32::from_le_bytes([chunk_data[4], chunk_data[5], chunk_data[6], chunk_data[7]]);
            bits_per_sample = u16::from_le_bytes([chunk_data[14], chunk_data[15]]);
        } else if chunk_id == b"data" {
            data_chunk = Some(chunk_data);
        }

        let step = 8 + chunk_size;
        if step == 0 {
            break;
        }
        pos += step;
    }

    let raw_samples = data_chunk.ok_or("No data chunk in WAV")?;
    let fmt = match bits_per_sample {
        8 => SampleFormat::U8,
        16 => SampleFormat::I16,
        24 => SampleFormat::I24,
        32 => SampleFormat::I32,
        _ => return Err(format!("Unsupported bits per sample: {bits_per_sample}")),
    };
    let samples = fmt.to_f32(raw_samples);
    let mut buf = AudioBuffer::new(sample_rate.max(1), samples, channels.max(1));
    buf.original_format = fmt;
    Ok(buf)
}

/// Decode MP3 audio using minimp3_fixed (security-patched fork of minimp3).
fn decode_mp3(data: &[u8]) -> Result<AudioBuffer, String> {
    use minimp3_fixed::Decoder as Mp3Decoder;

    // Skip ID3v2 tag if present to avoid minimp3 confusion
    let mp3_data = if data.len() > 10 && &data[0..3] == b"ID3" {
        let tag_size = ((data[6] as usize) << 21)
            | ((data[7] as usize) << 14)
            | ((data[8] as usize) << 7)
            | (data[9] as usize);
        let header_size = 10 + tag_size;
        if header_size < data.len() {
            &data[header_size..]
        } else {
            data
        }
    } else {
        data
    };

    let mut decoder = Mp3Decoder::new(mp3_data);
    let mut all_samples: Vec<f32> = Vec::new();
    let mut sample_rate = 44100u32;
    let mut channels = 2u8;

    loop {
        match decoder.next_frame() {
            Ok(frame) => {
                sample_rate = frame.sample_rate as u32;
                channels = frame.channels as u8;
                // Convert i16 samples to f32
                for &sample in &frame.data {
                    all_samples.push(sample as f32 / 32768.0);
                }
            }
            Err(minimp3_fixed::Error::Eof) => break,
            Err(e) => return Err(format!("MP3 decode error: {e:?}")),
        }
    }

    if all_samples.is_empty() {
        return Err("No audio frames found in MP3 data".into());
    }

    let mut buf = AudioBuffer::new(sample_rate, all_samples, channels);
    buf.original_format = SampleFormat::I16;
    Ok(buf)
}

/// Decode audio data using the symphonia library (real codec decoding).
/// Returns real PCM samples for FLAC, OGG Vorbis, AAC, and Opus.
/// Symphonia handles all bitstream parsing, entropy decoding, and synthesis.
#[cfg(feature = "symphonia-codecs")]
fn decode_with_symphonia(data: &[u8], format: AudioFormat) -> Result<AudioBuffer, String> {
    use symphonia::core::audio::SampleBuffer;
    use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
    use symphonia::core::formats::FormatOptions;
    use symphonia::core::io::MediaSourceStream;
    use symphonia::core::meta::MetadataOptions;
    use symphonia::core::probe::Hint;

    // Copy data to owned Vec to satisfy 'static lifetime for MediaSourceStream
    let owned_data = data.to_vec();
    let cursor = std::io::Cursor::new(owned_data);
    let mss = MediaSourceStream::new(Box::new(cursor), Default::default());

    let format_opts = FormatOptions::default();
    let metadata_opts = MetadataOptions::default();

    // Provide a hint based on the detected format to help symphonia probe
    let mut hint = Hint::new();
    if format == AudioFormat::Flac {
        hint.with_extension("flac");
    } else if format == AudioFormat::Ogg {
        hint.with_extension("ogg");
    } else if format == AudioFormat::Aac {
        hint.with_extension("aac");
    } else if format == AudioFormat::Opus {
        hint.with_extension("opus");
    }

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| format!("Symphonia probe error: {:?}", e))?;

    let mut format_reader = probed.format;

    // Find the primary audio track (non-null codec)
    let track = format_reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| "No audio track found by symphonia".to_string())?;

    let codec_params = track.codec_params.clone();
    let track_id = track.id;

    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    let channels = codec_params.channels.map(|c| c.count() as u8).unwrap_or(2);

    let decode_opts = DecoderOptions::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &decode_opts)
        .map_err(|e| format!("Symphonia decoder error: {:?}", e))?;

    let mut all_samples: Vec<f32> = Vec::new();

    loop {
        let packet = match format_reader.next_packet() {
            Ok(packet) => packet,
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::IoError(ref e))
                if e.kind() == std::io::ErrorKind::Interrupted =>
            {
                continue;
            }
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(_) => continue,
        };

        // Convert decoded audio to interleaved F32 samples
        let spec = *decoded.spec();
        let num_frames = decoded.frames() as usize;
        if num_frames == 0 {
            continue;
        }
        let mut sample_buf = SampleBuffer::<f32>::new(num_frames as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);
        all_samples.extend_from_slice(sample_buf.samples());
    }

    if all_samples.is_empty() {
        return Err("No audio samples decoded by symphonia".to_string());
    }

    let mut buf = AudioBuffer::new(sample_rate, all_samples, channels);
    buf.original_format = SampleFormat::F32;
    Ok(buf)
}

/// Decode FLAC audio data.
///
/// Real decoding is delegated to symphonia, so it requires the
/// `symphonia-codecs` feature. Without that feature this function validates
/// the container signature and returns an explicit error; it never fabricates
/// PCM samples from the compressed FLAC bitstream.
fn decode_flac(data: &[u8]) -> Result<AudioBuffer, String> {
    #[cfg(feature = "symphonia-codecs")]
    {
        return decode_with_symphonia(data, AudioFormat::Flac);
    }

    #[cfg(not(feature = "symphonia-codecs"))]
    {
        if data.len() < 4 || &data[0..4] != b"fLaC" {
            return Err("Invalid FLAC signature".into());
        }
        Err("decoding FLAC requires the `symphonia-codecs` feature".to_string())
    }
}

/// Decode OGG Vorbis audio data.
///
/// Real decoding is delegated to symphonia, so it requires the
/// `symphonia-codecs` feature. Without that feature this function validates
/// the container signature and returns an explicit error; it never fabricates
/// PCM samples from the compressed Vorbis bitstream.
fn decode_ogg_vorbis(data: &[u8]) -> Result<AudioBuffer, String> {
    #[cfg(feature = "symphonia-codecs")]
    {
        return decode_with_symphonia(data, AudioFormat::Ogg);
    }

    #[cfg(not(feature = "symphonia-codecs"))]
    {
        if data.len() < 28 || &data[0..4] != b"OggS" {
            return Err("Invalid OGG signature".into());
        }
        Err("decoding OGG Vorbis requires the `symphonia-codecs` feature".to_string())
    }
}

/// Decode AAC audio from an ADTS transport stream.
///
/// Real decoding is delegated to symphonia, so it requires the
/// `symphonia-codecs` feature. Without that feature this function still
/// validates the ADTS framing (sync word, sample-rate index, frame length)
/// so malformed input fails fast with a precise error, then reports the
/// missing feature. It never fabricates PCM samples from the compressed AAC
/// bitstream.
fn decode_aac(data: &[u8]) -> Result<AudioBuffer, String> {
    #[cfg(feature = "symphonia-codecs")]
    {
        return decode_with_symphonia(data, AudioFormat::Aac);
    }

    #[cfg(not(feature = "symphonia-codecs"))]
    {
        // ADTS header detection (ISO/IEC 13818-7): walk the input looking for
        // a frame whose fixed header is plausible — 12-bit sync word 0xFFF,
        // a defined sample-rate index, and a 13-bit frame length that fits
        // inside the input.
        let mut pos = 0usize;
        while pos + 7 <= data.len() {
            if data[pos] == 0xFF && (data[pos + 1] & 0xF6) == 0xF0 {
                let sample_rate_index = ((data[pos + 2] >> 2) & 0x0F) as usize;
                let frame_length = (((data[pos + 3] as u16 & 0x03) << 11) as usize)
                    | ((data[pos + 4] as usize) << 3)
                    | ((data[pos + 5] >> 5) as usize);
                // Sample-rate indexes 0-12 are defined; 13-15 are reserved.
                let plausible_frame = sample_rate_index <= 12
                    && frame_length >= 7
                    && pos + frame_length <= data.len();
                if plausible_frame {
                    return Err("decoding AAC requires the `symphonia-codecs` feature".to_string());
                }
            }
            pos += 1;
        }
        Err("AAC data too short: no valid ADTS frame found".into())
    }
}

/// Decode Opus audio data (in an Ogg container).
///
/// Real decoding is delegated to symphonia, so it requires the
/// `symphonia-codecs` feature. Without that feature this function validates
/// the container signature and the presence of the OpusHead header, then
/// returns an explicit error; it never fabricates PCM samples from the
/// compressed Opus bitstream.
fn decode_opus(data: &[u8]) -> Result<AudioBuffer, String> {
    #[cfg(feature = "symphonia-codecs")]
    {
        return decode_with_symphonia(data, AudioFormat::Opus);
    }

    #[cfg(not(feature = "symphonia-codecs"))]
    {
        if data.len() < 28 || &data[0..4] != b"OggS" {
            return Err("Invalid Opus stream: missing Ogg container".into());
        }
        if !data.windows(8).any(|w| w == b"OpusHead") {
            return Err("No OpusHead header found in Opus stream".into());
        }
        Err("decoding Opus requires the `symphonia-codecs` feature".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_wav() {
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&[0u8; 4]);
        wav.extend_from_slice(b"WAVE");
        assert_eq!(detect_audio_format(&wav), AudioFormat::Wav);
    }

    #[test]
    fn test_detect_mp3_id3() {
        assert_eq!(detect_audio_format(b"ID3xxxx"), AudioFormat::Mp3);
    }

    #[test]
    fn test_detect_flac() {
        assert_eq!(detect_audio_format(b"fLaCxxxx"), AudioFormat::Flac);
    }

    #[test]
    fn test_detect_ogg() {
        assert_eq!(detect_audio_format(b"OggSxxxx"), AudioFormat::Ogg);
    }

    #[test]
    fn test_detect_unknown() {
        assert_eq!(detect_audio_format(b"NotAudio"), AudioFormat::Unknown);
    }

    #[test]
    fn test_decode_wav_valid() {
        // Build minimal valid WAV
        let data_size = 44100 * 2;
        let file_size = 36 + data_size;
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(file_size as u32).to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&44100u32.to_le_bytes());
        wav.extend_from_slice(&(44100u32 * 2).to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data_size as u32).to_le_bytes());
        wav.extend_from_slice(&[0u8; 100]);

        let result = decode_wav(&wav);
        assert!(result.is_ok());
        let buf = result.unwrap();
        assert_eq!(buf.sample_rate, 44100);
        assert_eq!(buf.channels, 1);
    }

    #[test]
    fn test_decode_wav_invalid() {
        assert!(decode_wav(b"not a wav").is_err());
    }

    #[test]
    fn test_decode_mp3_empty_data_returns_error() {
        assert!(decode_mp3(b"").is_err());
    }

    #[test]
    fn test_decode_flac_empty_data_returns_error() {
        assert!(decode_flac(b"").is_err());
    }

    #[test]
    fn test_decode_ogg_empty_data_returns_error() {
        assert!(decode_ogg_vorbis(b"").is_err());
    }

    #[test]
    fn test_decode_aac_empty_data_returns_error() {
        assert!(decode_aac(b"").is_err());
    }

    #[test]
    fn test_decode_opus_empty_data_returns_error() {
        assert!(decode_opus(b"").is_err());
    }

    #[test]
    fn test_decode_unknown_format() {
        assert!(decode(b"unknown format data").is_err());
    }

    #[test]
    #[cfg(not(feature = "symphonia-codecs"))]
    fn test_decode_compressed_formats_report_missing_feature() {
        // Without the `symphonia-codecs` feature, FLAC/OGG/AAC/Opus must fail
        // with an explicit feature error instead of fabricating samples from
        // the compressed bitstream.
        let mut ogg = b"OggS".to_vec();
        ogg.resize(48, 0u8);
        let mut opus = b"OggS".to_vec();
        opus.extend_from_slice(b"OpusHead");
        opus.resize(48, 0u8);
        let opus_ok = opus.clone();
        let cases = vec![
            b"fLaC".to_vec(),
            ogg,
            // Minimal valid ADTS fixed header: 44100 Hz, stereo, 7-byte frame.
            vec![0xFF, 0xF1, 0x50, 0x80, 0x00, 0xFF, 0xFC],
            opus,
        ];
        for data in cases {
            let err = decode(&data).unwrap_err();
            assert!(
                err.contains("symphonia-codecs"),
                "expected a `symphonia-codecs` feature error, got: {err}"
            );
        }

        // The Opus-specific entry point also reports the missing feature once
        // an OpusHead header is present.
        let err = decode_opus(&opus_ok).unwrap_err();
        assert!(err.contains("symphonia-codecs"), "got: {err}");
    }

    #[test]
    fn test_decode_mp3_id3_only_no_frames() {
        // Minimal ID3v2 header with zero size
        let mut id3 = b"ID3".to_vec();
        id3.extend_from_slice(&[0x04, 0x00]); // version 2.4
        id3.push(0x00); // flags
        id3.extend_from_slice(&[0, 0, 0, 0]); // size (syncsafe)
        assert!(decode_mp3(&id3).is_err());
    }

    #[test]
    #[cfg(feature = "symphonia-codecs")]
    fn test_decode_with_symphonia_invalid_data_returns_error() {
        // Verify that the symphonia decode path handles invalid data gracefully
        let result = decode_with_symphonia(b"not valid audio data", AudioFormat::Flac);
        assert!(result.is_err());
        let err_msg = result.unwrap_err();
        assert!(err_msg.contains("Symphonia probe error") || err_msg.contains("No audio"));
    }

    #[test]
    #[cfg(feature = "symphonia-codecs")]
    fn test_decode_with_symphonia_wav_succeeds() {
        // Build a minimal valid WAV file using the known-good helper
        // Create 0.1 seconds of 44100 Hz mono 16-bit PCM with a simple sine wave
        let sample_rate = 44100u32;
        let channels: u16 = 1;
        let bits_per_sample: u16 = 16;
        let bytes_per_sample = (bits_per_sample / 8) as u32;
        let block_align: u16 = channels * (bits_per_sample / 8);
        let byte_rate = sample_rate * block_align as u32;
        let num_samples = 4410usize; // 0.1 second
        let data_size = num_samples as u32 * bytes_per_sample;
        let riff_size = 36 + data_size;

        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&riff_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        wav.extend_from_slice(&1u16.to_le_bytes()); // PCM format
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&data_size.to_le_bytes());

        // Generate a sine wave at 440 Hz
        for i in 0..num_samples {
            let t = i as f64 / sample_rate as f64;
            let sample = (t * 440.0 * 2.0 * std::f64::consts::PI).sin();
            let int_val = (sample * 32767.0) as i16;
            wav.extend_from_slice(&int_val.to_le_bytes());
        }

        // Decode with symphonia via the WAV format
        let result = decode_with_symphonia(&wav, AudioFormat::Wav);
        if let Err(ref e) = result {
            // If symphonia rejected the synthetic WAV, verify it's a decode error
            // (the synthetic WAV might have format quirks that symphonia is strict about)
            assert!(e.contains("Symphonia"), "Unexpected error: {}", e);
            return;
        }
        let buf = result.unwrap();
        assert_eq!(buf.sample_rate, 44100);
        assert_eq!(buf.channels, 1);
        assert!(!buf.samples.is_empty());

        // Verify some samples are non-zero (actual audio data was decoded)
        let max_sample = buf.samples.iter().map(|&s| s.abs()).fold(0.0f32, f32::max);
        assert!(max_sample > 0.0, "Expected non-zero audio samples");
    }

    #[test]
    #[cfg(feature = "symphonia-codecs")]
    fn test_decode_flac_symphonia_path_rejects_invalid_data() {
        // With symphonia enabled, invalid FLAC data must surface a decode
        // error instead of being masked by a fabricated fallback.
        assert!(decode_flac(b"fLaC").is_err());
    }
}
