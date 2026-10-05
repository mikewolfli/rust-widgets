// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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
    // WAV: RIFF....WAVE
    if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WAVE" {
        return AudioFormat::Wav;
    }
    // FLAC: fLaC
    if data.len() >= 4 && &data[0..4] == b"fLaC" {
        return AudioFormat::Flac;
    }
    // OGG: OggS
    if data.len() >= 4 && &data[0..4] == b"OggS" {
        return AudioFormat::Ogg;
    }
    // MP3: ID3 tag
    if data.len() >= 3 && &data[0..3] == b"ID3" {
        return AudioFormat::Mp3;
    }
    // MPEG audio (MP3) and AAC (ADTS) both start with 0xFF. They are told apart
    // by the second byte, never by the sync word alone:
    //   * ADTS has a 12-bit sync (0xFFF) and zero layer bits;
    //   * MPEG audio has an 11-bit sync (0xFFE), a non-zero layer, and a
    //     defined (non-reserved) MPEG version.
    if data.len() >= 2 && data[0] == 0xFF {
        // AAC ADTS: top 4 bits are 1111 and layer bits (bits 2-1) are 00.
        if (data[1] & 0xF6) == 0xF0 {
            return AudioFormat::Aac;
        }
        // MP3: top 3 bits are 111, layer bits are non-zero (Layer I/II/III),
        // and the MPEG version is not the reserved value (01). This keeps the
        // whole 0xF high nibble (MPEG-1) reachable instead of excluding it.
        if (data[1] & 0xE0) == 0xE0 && (data[1] & 0x06) != 0x00 && (data[1] & 0x18) != 0x08 {
            return AudioFormat::Mp3;
        }
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
            if !data.len().is_multiple_of(4) {
                return Err(format!(
                    "raw PCM data is {} bytes, which is not a whole number of 32-bit \
                     little-endian f32 samples; pad it to a multiple of 4 bytes",
                    data.len()
                ));
            }
            let samples: Vec<f32> = data
                .as_chunks::<4>()
                .0
                .iter()
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
    let mut channels = 0u16;
    let mut bits_per_sample = 0u16;
    let mut format_tag = 0u16;
    let mut data_chunk: Option<&[u8]> = None;

    while pos + 8 <= data.len() {
        let chunk_id = &data[pos..pos + 4];
        let chunk_size =
            u32::from_le_bytes([data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]])
                as usize;
        let chunk_start = pos + 8;
        let chunk_end = chunk_start.checked_add(chunk_size).ok_or("WAV chunk size overflow")?;
        if chunk_end > data.len() {
            return Err(format!(
                "WAV chunk {:?} truncated: need {chunk_size} bytes, got {}",
                chunk_id,
                data.len().saturating_sub(chunk_start)
            ));
        }
        let chunk_data = &data[chunk_start..chunk_end];

        if chunk_id == b"fmt " && chunk_data.len() >= 16 {
            format_tag = u16::from_le_bytes([chunk_data[0], chunk_data[1]]);
            // Kept as `u16` here: narrowing to `u8` would silently accept
            // channel counts like 257 as 1 and reject 256 as 0, i.e. treat two
            // equally-legal `u16` field values in completely different ways. The
            // count is validated explicitly after the chunk loop instead.
            channels = u16::from_le_bytes([chunk_data[2], chunk_data[3]]);
            sample_rate =
                u32::from_le_bytes([chunk_data[4], chunk_data[5], chunk_data[6], chunk_data[7]]);
            bits_per_sample = u16::from_le_bytes([chunk_data[14], chunk_data[15]]);
            // WAVE_FORMAT_EXTENSIBLE (0xFFFE) stores the real format tag in the
            // first two bytes of the SubFormat GUID, at offset 24 of a 40-byte
            // fmt chunk. Resolve it here so the code below can key off the
            // effective tag instead of guessing from bits_per_sample.
            if format_tag == 0xFFFE {
                if chunk_data.len() < 40 {
                    return Err("WAV fmt chunk uses WAVE_FORMAT_EXTENSIBLE but is too short \
                         (needs 40 bytes to carry the SubFormat GUID)"
                        .into());
                }
                format_tag = u16::from_le_bytes([chunk_data[24], chunk_data[25]]);
            }
        } else if chunk_id == b"data" {
            data_chunk = Some(chunk_data);
        }

        let step = 8usize
            .checked_add(chunk_size)
            .and_then(|size| size.checked_add(chunk_size % 2))
            .ok_or("WAV chunk offset overflow")?;
        pos = pos.checked_add(step).ok_or("WAV chunk offset overflow")?;
    }

    if sample_rate == 0 || channels == 0 {
        return Err("WAV fmt chunk is missing or invalid".into());
    }
    // Reject unsupported channel counts explicitly (N-S-51). The buffer stores
    // channels in a `u8`, so anything above `u8::MAX` cannot be represented; a
    // 0 channel count is meaningless. Both are reported rather than narrowed,
    // so `257` and `256` fail the same way instead of one becoming mono and the
    // other being rejected by a separate `channels == 0` check.
    let channels_u8 = u8::try_from(channels)
        .map_err(|_| format!("unsupported WAV channel count: {channels} (must be 1..=255)"))?;
    let raw_samples = data_chunk.ok_or("No data chunk in WAV")?;
    let fmt = sample_format_for_tag(format_tag, bits_per_sample)?;
    let bytes_per_sample = fmt.bytes_per_sample();
    if raw_samples.len() % bytes_per_sample != 0 {
        return Err(format!(
            "WAV data chunk is {} bytes, which is not a whole number of {bits_per_sample}-bit \
             samples ({} bytes each); the data chunk is truncated",
            raw_samples.len(),
            bytes_per_sample
        ));
    }
    // A WAV data chunk holds whole *frames*, so its length must be a multiple of
    // one frame (channels × bytes per sample). Checking only per sample would
    // accept a stereo file with a single sample, which is half a frame and would
    // make every downstream interleaved read misaligned (N-S-51).
    let bytes_per_frame = bytes_per_sample * channels as usize;
    if raw_samples.len() % bytes_per_frame != 0 {
        return Err(format!(
            "WAV data chunk is {} bytes, which is not a whole number of {channels}-channel \
             frames ({bytes_per_frame} bytes each); the data chunk holds an incomplete final frame",
            raw_samples.len()
        ));
    }
    let samples = fmt.to_f32(raw_samples);
    let mut buf = AudioBuffer::new(sample_rate.max(1), samples, channels_u8);
    buf.original_format = fmt;
    Ok(buf)
}

/// Map a WAV `wFormatTag` (plus `wBitsPerSample`) to a [`SampleFormat`].
///
/// Integer PCM (tag 1) is distinguished from IEEE float (tag 3); unsupported or
/// compressed tags are rejected with an explicit error instead of being decoded
/// as if they were integer PCM.
fn sample_format_for_tag(format_tag: u16, bits_per_sample: u16) -> Result<SampleFormat, String> {
    match format_tag {
        // WAVE_FORMAT_PCM
        0x0001 => match bits_per_sample {
            8 => Ok(SampleFormat::U8),
            16 => Ok(SampleFormat::I16),
            24 => Ok(SampleFormat::I24),
            32 => Ok(SampleFormat::I32),
            _ => Err(format!("unsupported PCM bits per sample: {bits_per_sample}")),
        },
        // WAVE_FORMAT_IEEE_FLOAT
        0x0003 => {
            if bits_per_sample == 32 {
                Ok(SampleFormat::F32)
            } else {
                Err(format!(
                    "unsupported IEEE float bits per sample: {bits_per_sample} \
                     (only 32-bit float is supported)"
                ))
            }
        }
        other => Err(format!("unsupported WAV format tag: {other:#06x}")),
    }
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
            Err(e) => {
                return Err(format!(
                    "MP3 frame after {} decoded sample(s) could not be decoded: {e:?}",
                    all_samples.len()
                ))
            }
        }
    }

    if all_samples.is_empty() {
        return Err(format!(
            "MP3 decoded zero audio frames from {} bytes: the stream has no usable MPEG frame \
             (check that the data is really MP3 and not a raw PCM or container payload)",
            data.len()
        ));
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
        .map_err(|e| {
        format!(
            "symphonia could not probe the stream container (unsupported or corrupt format): {e:?}"
        )
    })?;

    let mut format_reader = probed.format;

    // Find the primary audio track (non-null codec)
    let track = format_reader
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| {
            format!(
                "the stream has no decodable audio track: symphonia found {} track(s), none with \
                 a real codec — the container may hold only video or metadata",
                format_reader.tracks().len()
            )
        })?;

    let codec_params = track.codec_params.clone();
    let track_id = track.id;

    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    let channels = codec_params.channels.map(|c| c.count() as u8).unwrap_or(2);

    let decode_opts = DecoderOptions::default();
    let mut decoder =
        symphonia::default::get_codecs().make(&codec_params, &decode_opts).map_err(|e| {
            format!(
                "symphonia could not build a decoder for codec {codec:?}: {e:?} (the codec may \
                 require a different `symphonia-*` feature)",
                codec = codec_params.codec
            )
        })?;

    let mut all_samples: Vec<f32> = Vec::new();

    // Recovery policy for mid-stream errors. Symphonia distinguishes *expected*
    // outcomes (a normal end of stream, a retryable interrupt) from *genuine*
    // decode failures. The old code collapsed everything else into a bare
    // `break`/`continue`, which silently turned a corrupt file into a partial
    // success: the caller got an `Ok(AudioBuffer)` truncated at the fault with no
    // indication that anything was lost. Both the packet reader and the decoder
    // now classify each error through [`RecoverableError`] and, on an
    // unrecoverable one, surface it as an explicit `Err` rather than a silent
    // short read.
    loop {
        let packet = match format_reader.next_packet() {
            Ok(packet) => packet,
            Err(error) => match classify_symphonia_error(&error) {
                // A clean end of stream is not an error; stop normally.
                RecoverableError::EndOfStream => break,
                // An interrupt is transient: retry the read.
                RecoverableError::Skip => continue,
                RecoverableError::Unrecoverable(reason) => {
                    return Err(format!(
                        "audio stream could not be read after {} decoded sample(s): {reason}",
                        all_samples.len()
                    ));
                }
            },
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(error) => match classify_symphonia_error(&error) {
                // The decoder reported the end of its input; stop normally.
                RecoverableError::EndOfStream => break,
                // A transient decode hiccup: skip this packet and keep going.
                RecoverableError::Skip => continue,
                // A packet genuinely failed to decode. Delivering the samples
                // decoded so far as if they were the whole stream would hide
                // corruption, so report it explicitly.
                RecoverableError::Unrecoverable(reason) => {
                    return Err(format!(
                        "audio packet at {} decoded sample(s) could not be decoded: {reason}",
                        all_samples.len()
                    ));
                }
            },
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
        return Err(format!(
            "symphonia decoded zero audio samples from {} bytes: every packet was empty or \
             skipped, so there is no PCM to return",
            data.len()
        ));
    }

    let mut buf = AudioBuffer::new(sample_rate, all_samples, channels);
    buf.original_format = SampleFormat::F32;
    Ok(buf)
}

/// How a symphonia error should be treated by the decode loop.
///
/// Symphonia's `Error` enum mixes three very different situations behind one
/// type, so the loop must decide per error whether to stop cleanly, retry, or
/// fail. Modelling that decision as a value keeps the policy in one place and
/// makes it unit-testable without manufacturing a corrupt media file.
#[cfg(feature = "symphonia-codecs")]
#[derive(Debug, Clone, PartialEq, Eq)]
enum RecoverableError {
    /// The stream ended normally; the samples decoded so far are complete.
    EndOfStream,
    /// A transient condition (`Interrupted`, or the decoder asking for more
    /// input); retry rather than abort.
    Skip,
    /// A genuine failure. Carries a human-readable reason; the caller must
    /// report it instead of returning a silently truncated buffer.
    Unrecoverable(String),
}

/// Classifies a symphonia error against the decode loop's recovery policy.
///
/// * `IoError(UnexpectedEof)` is a clean end of stream — the samples collected
///   so far are the whole usable stream. (Symphonia reports end of stream as an
///   `IoError` with `UnexpectedEof`, not a dedicated variant.)
/// * `IoError(Interrupted)` and `ResetRequired` are transient; the caller should
///   retry the same step.
/// * Everything else is unrecoverable and is reported with a reason string.
#[cfg(feature = "symphonia-codecs")]
fn classify_symphonia_error(error: &symphonia::core::errors::Error) -> RecoverableError {
    use symphonia::core::errors::Error;
    match error {
        Error::IoError(e) if e.kind() == std::io::ErrorKind::Interrupted => RecoverableError::Skip,
        Error::IoError(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
            RecoverableError::EndOfStream
        }
        // The decoder needs its state reset before it can continue; that is a
        // caller mistake here (we never request a reset) but it is recoverable in
        // principle, so it is treated as a retry rather than a hard failure.
        Error::ResetRequired => RecoverableError::Skip,
        other => RecoverableError::Unrecoverable(format!("{other:?}")),
    }
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
        decode_with_symphonia(data, AudioFormat::Flac)
    }

    #[cfg(not(feature = "symphonia-codecs"))]
    {
        if data.len() < 4 || &data[0..4] != b"fLaC" {
            return Err(format!(
                "FLAC must start with the magic \"fLaC\", but this {}-byte input starts with \
                 {:02X?}",
                data.len(),
                &data[..data.len().min(4)]
            ));
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
        decode_with_symphonia(data, AudioFormat::Ogg)
    }

    #[cfg(not(feature = "symphonia-codecs"))]
    {
        if data.len() < 28 || &data[0..4] != b"OggS" {
            return Err(format!(
                "OGG must start with the capture pattern \"OggS\", but this {}-byte input \
                 starts with {:02X?}",
                data.len(),
                &data[..data.len().min(4)]
            ));
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
        decode_with_symphonia(data, AudioFormat::Aac)
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
        Err(format!(
            "AAC data ({} bytes) contains no valid ADTS frame: every 0xFFF sync candidate \
             failed the header checks",
            data.len()
        ))
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
        decode_with_symphonia(data, AudioFormat::Opus)
    }

    #[cfg(not(feature = "symphonia-codecs"))]
    {
        if data.len() < 28 || &data[0..4] != b"OggS" {
            return Err(format!(
                "Opus must be wrapped in an Ogg container starting with \"OggS\", but this \
                 {}-byte input starts with {:02X?}",
                data.len(),
                &data[..data.len().min(4)]
            ));
        }
        if !data.windows(8).any(|w| w == b"OpusHead") {
            return Err(format!(
                "Opus stream has no OpusHead identification header in its {} bytes: the Ogg \
                 container holds no Opus logical bitstream",
                data.len()
            ));
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
    fn test_detect_mp3_mpeg1_layer3_without_id3() {
        // A common MPEG-1 Layer III header `FF FB 90 64` must not be misread as
        // ADTS AAC, even though its second-byte high nibble is 0xF.
        assert_eq!(detect_audio_format(&[0xFF, 0xFB, 0x90, 0x64]), AudioFormat::Mp3);
    }

    #[test]
    fn test_detect_mp3_mpeg1_all_layers() {
        // MPEG-1 (version 11) Layer III (01), Layer II (10), and Layer I (11).
        assert_eq!(detect_audio_format(&[0xFF, 0xFB, 0x00, 0x00]), AudioFormat::Mp3);
        assert_eq!(detect_audio_format(&[0xFF, 0xFD, 0x00, 0x00]), AudioFormat::Mp3);
        assert_eq!(detect_audio_format(&[0xFF, 0xFF, 0x00, 0x00]), AudioFormat::Mp3);
    }

    #[test]
    fn test_detect_mp3_mpeg2_and_mpeg25() {
        // MPEG-2 Layer III (version 10) and MPEG-2.5 Layer III (version 00).
        assert_eq!(detect_audio_format(&[0xFF, 0xF3, 0x00, 0x00]), AudioFormat::Mp3);
        assert_eq!(detect_audio_format(&[0xFF, 0xE3, 0x00, 0x00]), AudioFormat::Mp3);
    }

    #[test]
    fn test_detect_aac_adts() {
        // ADTS: 12-bit sync 0xFFF with zero layer bits.
        assert_eq!(
            detect_audio_format(&[0xFF, 0xF1, 0x50, 0x80, 0x00, 0xFF, 0xFC]),
            AudioFormat::Aac
        );
        // MPEG-4 (version 0) and MPEG-2 (version 1) ADTS variants.
        assert_eq!(detect_audio_format(&[0xFF, 0xF0, 0x00, 0x00]), AudioFormat::Aac);
        assert_eq!(detect_audio_format(&[0xFF, 0xF8, 0x00, 0x00]), AudioFormat::Aac);
    }

    #[test]
    fn test_detect_short_or_reserved_headers_are_not_guessed() {
        // Sync alone is never enough.
        assert_eq!(detect_audio_format(&[0xFF]), AudioFormat::Unknown);
        assert_eq!(detect_audio_format(&[]), AudioFormat::Unknown);
        // Reserved MPEG layer (00) must not be guessed as MP3.
        assert_eq!(detect_audio_format(&[0xFF, 0xE1, 0x00, 0x00]), AudioFormat::Unknown);
        // Reserved MPEG version (01) must not be guessed as MP3.
        assert_eq!(detect_audio_format(&[0xFF, 0xEF, 0x00, 0x00]), AudioFormat::Unknown);
    }

    #[test]
    fn test_decode_wav_valid() {
        // Build minimal valid WAV
        let data_size = 100;
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
        assert_eq!(buf.channels(), 1);
    }

    #[test]
    fn test_decode_wav_rejects_truncated_chunk() {
        let mut wav = b"RIFF".to_vec();
        wav.extend_from_slice(&42u32.to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&44100u32.to_le_bytes());
        wav.extend_from_slice(&88200u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&100u32.to_le_bytes());
        wav.extend_from_slice(&[0u8; 2]);
        let error = decode_wav(&wav).unwrap_err();
        assert!(error.contains("truncated"), "unexpected error: {error}");
    }

    #[test]
    fn test_decode_wav_invalid() {
        assert!(decode_wav(b"not a wav").is_err());
    }

    /// Build a minimal WAV file with the given format tag, channels, sample
    /// rate and bit depth. For WAVE_FORMAT_EXTENSIBLE (0xFFFE) the SubFormat
    /// GUID is written with PCM (0x0001) as the embedded sub-format tag.
    fn build_wav(
        format_tag: u16,
        channels: u16,
        sample_rate: u32,
        bits_per_sample: u16,
        data: &[u8],
    ) -> Vec<u8> {
        let bytes_per_sample = bits_per_sample / 8;
        let block_align = channels * bytes_per_sample;
        let byte_rate = sample_rate * block_align as u32;
        let fmt_chunk_len: u32 = if format_tag == 0xFFFE { 40 } else { 16 };
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        let riff_size = 4 + (8 + fmt_chunk_len) + (8 + data.len() as u32);
        wav.extend_from_slice(&riff_size.to_le_bytes());
        wav.extend_from_slice(b"WAVE");
        wav.extend_from_slice(b"fmt ");
        wav.extend_from_slice(&fmt_chunk_len.to_le_bytes());
        wav.extend_from_slice(&format_tag.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        if format_tag == 0xFFFE {
            wav.extend_from_slice(&22u16.to_le_bytes()); // cbSize
            wav.extend_from_slice(&bits_per_sample.to_le_bytes()); // wValidBitsPerSample
            wav.extend_from_slice(&0u32.to_le_bytes()); // dwChannelMask
                                                        // KSDATAFORMAT_SUBTYPE_PCM GUID: first two bytes = 0x0001 (PCM).
            wav.extend_from_slice(&[
                0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38,
                0x9b, 0x71,
            ]);
        }
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&(data.len() as u32).to_le_bytes());
        wav.extend_from_slice(data);
        wav
    }

    #[test]
    fn test_decode_wav_ieee_float() {
        // format tag 3, 32-bit IEEE float. A 1.0 sample must decode to ~1.0,
        // not to the ~0.496 value produced by reading the float bits as i32.
        let mut data = Vec::new();
        for s in [1.0f32, -1.0f32, 0.5f32, 0.0f32] {
            data.extend_from_slice(&s.to_le_bytes());
        }
        let wav = build_wav(0x0003, 1, 44100, 32, &data);
        let buf = decode_wav(&wav).unwrap();
        assert_eq!(buf.sample_rate, 44100);
        assert_eq!(buf.channels(), 1);
        assert_eq!(buf.original_format, SampleFormat::F32);
        assert!((buf.samples[0] - 1.0).abs() < 1e-6, "got {}", buf.samples[0]);
        assert!((buf.samples[1] + 1.0).abs() < 1e-6, "got {}", buf.samples[1]);
        assert!((buf.samples[2] - 0.5).abs() < 1e-6, "got {}", buf.samples[2]);
        assert_eq!(buf.samples[3], 0.0);
    }

    #[test]
    fn test_decode_wav_rejects_unsupported_format_tag() {
        // A-law (0x0006) is compressed and must not be decoded as integer PCM.
        let wav = build_wav(0x0006, 1, 44100, 8, &[0x80]);
        let err = decode_wav(&wav).unwrap_err();
        assert!(err.contains("format tag"), "got: {err}");
    }

    #[test]
    fn test_decode_wav_rejects_unsupported_float_bit_depth() {
        let wav = build_wav(0x0003, 1, 44100, 64, &[0u8; 8]);
        let err = decode_wav(&wav).unwrap_err();
        assert!(err.contains("IEEE float"), "got: {err}");
    }

    #[test]
    fn test_decode_wav_extensible_pcm() {
        // 16-bit PCM wrapped in WAVE_FORMAT_EXTENSIBLE must decode as I16.
        let mut data = Vec::new();
        for s in [0i16, 32767, -32768] {
            data.extend_from_slice(&s.to_le_bytes());
        }
        let wav = build_wav(0xFFFE, 1, 44100, 16, &data);
        let buf = decode_wav(&wav).unwrap();
        assert_eq!(buf.original_format, SampleFormat::I16);
        assert!((buf.samples[0] - 0.0).abs() < 0.01, "got {}", buf.samples[0]);
        assert!((buf.samples[1] - 32767.0 / 32768.0).abs() < 0.01, "got {}", buf.samples[1]);
        assert!((buf.samples[2] + 1.0).abs() < 0.01, "got {}", buf.samples[2]);
    }

    /// N-S-51: an out-of-range channel count must be rejected explicitly, and two
    /// legal `u16` field values that both exceed a `u8` must fail identically
    /// (the old `as u8` narrowed 257 to 1 and turned 256 into 0).
    #[test]
    fn test_decode_wav_rejects_unsupported_channel_counts_without_narrowing() {
        // 257 would have narrowed to 1 (mono) under the old code.
        let wav = build_wav(0x0001, 257, 44100, 16, &[0u8; 4]);
        let err = decode_wav(&wav).unwrap_err();
        assert!(err.contains("channel count"), "got: {err}");

        // 256 would have narrowed to 0 and been rejected by a *different* check;
        // the unified validation must reject it the same way.
        let wav = build_wav(0x0001, 256, 44100, 16, &[0u8; 4]);
        let err = decode_wav(&wav).unwrap_err();
        assert!(err.contains("channel count"), "got: {err}");
    }

    /// N-S-51: a stereo data chunk must contain whole frames. A single 16-bit
    /// sample is half a frame and previously slipped past the per-sample check.
    #[test]
    fn test_decode_wav_rejects_incomplete_final_frame() {
        // Stereo, 16-bit, one sample (2 bytes) = half of a 4-byte frame.
        let wav = build_wav(0x0001, 2, 44100, 16, &[0u8, 0]);
        let err = decode_wav(&wav).unwrap_err();
        assert!(err.contains("incomplete final frame"), "got: {err}");

        // A full frame (two samples, 4 bytes) decodes fine.
        let wav = build_wav(0x0001, 2, 44100, 16, &[0u8; 4]);
        let buf = decode_wav(&wav).unwrap();
        assert_eq!(buf.channels(), 2);
        assert_eq!(buf.samples.len(), 2);
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
        // A stream symphonia cannot even probe is reported through the probe
        // mapper; a stream that probes but has no usable track through the
        // track mapper. Either is an acceptable explicit failure.
        assert!(
            err_msg.contains("could not probe") || err_msg.contains("no decodable audio track"),
            "unexpected error: {err_msg}"
        );
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
        assert_eq!(buf.channels(), 1);
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

    /// The recovery policy must treat a clean end of stream and a transient
    /// error as non-fatal, and must mark every other error unrecoverable — the
    /// distinction the loop relies on to avoid returning a truncated buffer as a
    /// successful decode (N-S-47).
    #[test]
    #[cfg(feature = "symphonia-codecs")]
    fn test_classify_symphonia_error_policy() {
        use std::io::{Error as IoError, ErrorKind};
        use symphonia::core::errors::Error;

        assert_eq!(
            classify_symphonia_error(&Error::IoError(IoError::new(ErrorKind::UnexpectedEof, ""))),
            RecoverableError::EndOfStream,
            "UnexpectedEof means the stream ended cleanly, not a failure"
        );
        assert_eq!(
            classify_symphonia_error(&Error::IoError(IoError::new(ErrorKind::Interrupted, ""))),
            RecoverableError::Skip,
            "Interrupted is transient and must be retried"
        );
        assert_eq!(
            classify_symphonia_error(&Error::ResetRequired),
            RecoverableError::Skip,
            "ResetRequired is recoverable"
        );
        // A genuine decode failure must be reported, not swallowed.
        assert!(matches!(
            classify_symphonia_error(&Error::DecodeError("corrupt frame")),
            RecoverableError::Unrecoverable(_)
        ));
        assert!(matches!(
            classify_symphonia_error(&Error::IoError(IoError::new(ErrorKind::Other, ""))),
            RecoverableError::Unrecoverable(_)
        ));
    }
}
