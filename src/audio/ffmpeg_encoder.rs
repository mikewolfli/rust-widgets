//! Real audio encoding via FFmpeg (ffmpeg-next).
//! Gated behind `#[cfg(feature = "video-codecs")]`.
//!
//! Encodes `AudioBuffer` to MP3, FLAC, OGG/Vorbis, AAC, or Opus
//! using FFmpeg's libavcodec + libavformat.  The output is written
//! to a temporary file, then read back into memory.

use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ffmpeg_next::channel_layout::ChannelLayout;
use ffmpeg_next::format::sample::{Sample, Type as SampleType};
use ffmpeg_next::frame::Audio as AudioFrame;
use ffmpeg_next::software::resampling;
use ffmpeg_next::Packet;

use crate::audio::format::AudioFormat;
use crate::audio::samples::AudioBuffer;

// ---------------------------------------------------------------------------
// Atomic counter for unique temp-file names
// ---------------------------------------------------------------------------

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_temp_path(ext: &str) -> PathBuf {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let mut path = std::env::temp_dir();
    path.push(format!("rust_widgets_audio_enc_{pid}_{count}.{ext}"));
    path
}

// ---------------------------------------------------------------------------
// Format → (muxer-format, encoder-name, default-bitrate)
// ---------------------------------------------------------------------------

fn format_to_ffmpeg_params(
    format: AudioFormat,
) -> Result<(&'static str, &'static str, i64), String> {
    match format {
        AudioFormat::Mp3 => Ok(("mp3", "libmp3lame", 192_000)),
        AudioFormat::Flac => Ok(("flac", "flac", 0)),
        AudioFormat::Ogg => Ok(("ogg", "libvorbis", 128_000)),
        AudioFormat::Aac => Ok(("adts", "aac", 128_000)),
        AudioFormat::Opus => Ok(("opus", "libopus", 64_000)),
        _ => Err(format!("FFmpeg encoder does not support {:?}", format)),
    }
}

// ---------------------------------------------------------------------------
// Build an F32 interleaved frame from the audio buffer slice
// ---------------------------------------------------------------------------

/// Create an F32-packed `AudioFrame` filled with samples from `buffer`.
fn build_f32_frame(
    buffer: &AudioBuffer,
    buffer_offset: usize,
    frame_samples: usize,
    channel_layout: ChannelLayout,
    pts: i64,
    sample_rate: u32,
) -> Result<AudioFrame, String> {
    let channels = buffer.channels as usize;
    let mut frame = AudioFrame::new(Sample::F32(SampleType::Packed), frame_samples, channel_layout);
    frame.set_rate(sample_rate);
    frame.set_pts(Some(pts));

    let dst = frame.data_mut(0);
    let total = frame_samples * channels;
    let end = (buffer_offset + total).min(buffer.samples.len());
    let src = &buffer.samples[buffer_offset..end];
    let src_bytes = unsafe { std::slice::from_raw_parts(src.as_ptr() as *const u8, src.len() * 4) };
    let copy_len = dst.len().min(src_bytes.len());
    dst[..copy_len].copy_from_slice(&src_bytes[..copy_len]);
    Ok(frame)
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Encode an `AudioBuffer` to bytes in the given format using FFmpeg.
///
/// Only supports `Mp3`, `Flac`, `Ogg`, `Aac`, and `Opus`.
/// Returns a `String` error on failure.
pub fn ffmpeg_encode(buffer: &AudioBuffer, format: AudioFormat) -> Result<Vec<u8>, String> {
    ffmpeg_next::init().map_err(|e| format!("FFmpeg init failed: {e}"))?;

    let (muxer_name, encoder_name, bit_rate) = format_to_ffmpeg_params(format)?;
    let sample_rate = buffer.sample_rate as i32;

    // ── Temp output file ─────────────────────────────────────────────
    let ext = format.extension();
    let tmp_path = next_temp_path(ext);
    let path_str = tmp_path.to_str().ok_or("Invalid temp file path")?.to_owned();

    // ── Create output context (muxer) ────────────────────────────────
    let mut octx = ffmpeg_next::format::output_as(&path_str, muxer_name)
        .map_err(|e| format!("Failed to create muxer '{muxer_name}': {e}"))?;

    // Save global-header flag before we borrow octx via a stream.
    let global = octx.format().flags().contains(ffmpeg_next::format::flag::Flags::GLOBAL_HEADER);

    // ── Find encoder ─────────────────────────────────────────────────
    let codec_descriptor = ffmpeg_next::encoder::find_by_name(encoder_name)
        .ok_or_else(|| format!("Encoder '{encoder_name}' not found"))?;
    let codec_audio = codec_descriptor
        .audio()
        .map_err(|e| format!("'{encoder_name}' is not an audio encoder: {e}"))?;

    // ── Create encoder context independently ─────────────────────────
    // Build and open the encoder before touching the muxer stream so we
    // can avoid borrowing `octx` through a `StreamMut` while encoding.
    let encoder_ctx = ffmpeg_next::codec::context::Context::new_with_codec(codec_descriptor);
    let mut encoder_initial = encoder_ctx
        .encoder()
        .audio()
        .map_err(|e| format!("Failed to create audio encoder: {e}"))?;

    // ── Set encoder parameters ──────────────────────────────────────
    let channel_layout = match buffer.channels {
        1 => ChannelLayout::MONO,
        _ => ChannelLayout::STEREO,
    };

    // Determine supported sample rate (Opus only supports specific rates).
    let encoder_sample_rate = codec_audio
        .rates()
        .and_then(|mut r| r.find(|&rate| rate == sample_rate))
        .unwrap_or(codec_audio.rates().and_then(|mut r| r.next()).unwrap_or(sample_rate));

    if bit_rate > 0 {
        encoder_initial.set_bit_rate(bit_rate as usize);
    }
    encoder_initial.set_rate(encoder_sample_rate);
    encoder_initial.set_channel_layout(channel_layout);
    // Use the first format the encoder supports; the resampler handles
    // conversion from our F32 interleaved data.
    let sample_format =
        codec_audio.formats().and_then(|mut f| f.next()).unwrap_or(Sample::F32(SampleType::Packed));
    encoder_initial.set_format(sample_format);
    encoder_initial.set_time_base((1, encoder_sample_rate));

    if global {
        encoder_initial.set_flags(ffmpeg_next::codec::Flags::GLOBAL_HEADER);
    }

    // ── Open encoder ─────────────────────────────────────────────────
    let mut encoder = encoder_initial
        .open_as(codec_descriptor)
        .map_err(|e| format!("Failed to open encoder '{encoder_name}': {e}"))?;

    // Determine the actual sample format the encoder uses after opening
    let actual_format = encoder.format();

    // ── Add stream, associate encoder, save index ────────────────────
    let stream_index: usize;
    {
        let mut ost =
            octx.add_stream(codec_descriptor).map_err(|e| format!("Failed to add stream: {e}"))?;
        ost.set_parameters(&encoder);
        stream_index = ost.index();
    }

    // ── Write muxer header ───────────────────────────────────────────
    octx.write_header().map_err(|e| format!("Failed to write header: {e}"))?;

    // ── Set up resampler (F32 packed → encoder's native format/rate) ─
    let src_rate = sample_rate as u32;
    let src_layout = channel_layout;
    let src_format = Sample::F32(SampleType::Packed);
    let dst_rate = encoder_sample_rate as u32;
    let dst_layout = channel_layout;
    let dst_format = actual_format;

    let needs_resample = dst_format != src_format || dst_rate != src_rate;
    let mut resampler = if needs_resample {
        Some(
            resampling::Context::get(
                src_format, src_layout, src_rate, dst_format, dst_layout, dst_rate,
            )
            .map_err(|e| format!("Failed to create resampler: {e}"))?,
        )
    } else {
        None
    };

    // ── Encode ───────────────────────────────────────────────────────
    let channels_us = buffer.channels as usize;
    let frame_size = encoder.frame_size() as usize;
    // Use the encoder's preferred frame size, or 1024 as default for
    // variable-frame-size encoders (libvorbis, libopus).
    // For FLAC which returns 4096, we use that exact size.
    let samples_per_frame = if frame_size > 0 { frame_size } else { 1024 };

    let total_samples = buffer.samples.len();
    let mut sample_offset = 0;
    let mut pts: i64 = 0;

    while sample_offset < total_samples {
        let samples_remaining = total_samples - sample_offset;
        let frames_remaining = samples_remaining / channels_us;
        let this_frame_samples = samples_per_frame.min(frames_remaining);

        if this_frame_samples == 0 {
            break;
        }

        // Build an F32 packed frame (our native buffer format)
        let f32_frame = build_f32_frame(
            buffer,
            sample_offset,
            this_frame_samples,
            channel_layout,
            pts,
            sample_rate as u32,
        )?;

        // Convert to encoder's format via resampler if needed
        let frame_to_send = if let Some(ref mut resampler) = resampler {
            let mut converted = AudioFrame::empty();
            resampler
                .run(&f32_frame, &mut converted)
                .map_err(|e| format!("Resampler error: {e}"))?;
            // Preserve PTS so the encoder can stamp packets correctly
            converted.set_pts(f32_frame.pts());
            converted
        } else {
            f32_frame
        };

        // Send frame to encoder
        if let Err(e) = encoder.send_frame(&frame_to_send) {
            return Err(format!("Send frame error (pts={}): {}", pts, e));
        }

        // Receive all packets produced from this frame
        let mut packet = Packet::empty();
        loop {
            match encoder.receive_packet(&mut packet) {
                Ok(()) => {
                    packet.set_stream(stream_index);
                    packet
                        .write_interleaved(&mut octx)
                        .map_err(|e| format!("Write packet error: {e}"))?;
                }
                // Eof ends the stream; EAGAIN means the encoder has no packet
                // ready yet and is waiting for more input frames. Both are
                // normal here, so keep sending frames. Anything else is a
                // genuine encode failure and is propagated.
                Err(ffmpeg_next::Error::Eof) => break,
                Err(ffmpeg_next::Error::Other { errno })
                    if std::io::Error::from_raw_os_error(errno).kind()
                        == std::io::ErrorKind::WouldBlock =>
                {
                    break;
                }
                Err(e) => return Err(format!("Receive packet error (pts={pts}): {e}")),
            }
            packet = Packet::empty();
        }

        let samples_consumed = this_frame_samples * channels_us;
        sample_offset += samples_consumed;
        pts += this_frame_samples as i64;
    }

    // ── Flush encoder & write trailer ────────────────────────────────
    // Send EOF to switch the encoder into draining mode, then pull every
    // remaining packet until the encoder reports `Eof`.
    encoder.send_eof().map_err(|e| format!("Failed to flush encoder: {e}"))?;

    let is_flac = format == AudioFormat::Flac;
    let mut dropped_packets: usize = 0;
    let mut packet = Packet::empty();
    loop {
        match encoder.receive_packet(&mut packet) {
            Ok(()) => {
                packet.set_stream(stream_index);
                if let Err(e) = packet.write_interleaved(&mut octx) {
                    if is_flac {
                        // Workaround for FFmpeg's FLAC encoder producing
                        // trailing packets that the FLAC muxer rejects: drop
                        // them, but make the loss visible instead of
                        // swallowing it silently.
                        dropped_packets += 1;
                        log::warn!("FLAC flush: muxer rejected a trailing packet ({e}); dropped");
                    } else {
                        return Err(format!("Write packet error during flush: {e}"));
                    }
                }
            }
            Err(ffmpeg_next::Error::Eof) => break,
            // EAGAIN can surface while draining (the encoder has no more
            // packets right now); end the drain loop so the trailer can be
            // written. Genuine errors are propagated.
            Err(ffmpeg_next::Error::Other { errno })
                if std::io::Error::from_raw_os_error(errno).kind()
                    == std::io::ErrorKind::WouldBlock =>
            {
                break;
            }
            Err(e) => return Err(format!("Receive packet error during flush: {e}")),
        }
        packet = Packet::empty();
    }
    if dropped_packets > 0 {
        log::warn!("FLAC flush: dropped {dropped_packets} packet(s) rejected by the muxer");
    }

    octx.write_trailer().map_err(|e| format!("Write trailer error: {e}"))?;

    // ── Read back ────────────────────────────────────────────────────
    let result = fs::read(&tmp_path).map_err(|e| format!("Failed to read output file: {e}"))?;

    // Clean up temp file
    let _ = fs::remove_file(&tmp_path);

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioFormat;
    use crate::audio::samples::AudioBuffer;

    #[test]
    fn test_flac_encode_mono() {
        // 4096 samples mono = exactly 1 frame (FLAC's default frame_size=4096)
        let samples: Vec<f32> = (0..4096)
            .map(|i| (i as f32 / 44100.0 * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5)
            .collect();
        let buf = AudioBuffer::new(44100, samples, 1);
        let result = ffmpeg_encode(&buf, AudioFormat::Flac);
        assert!(result.is_ok(), "FLAC mono 1-frame encoding failed: {:?}", result);
        assert!(!result.unwrap().is_empty(), "FLAC mono output is empty");
    }

    #[test]
    fn test_flac_encode_stereo_two_frames() {
        // 16384 samples stereo = 8192 frames = 2 frames of 4096 each
        let samples: Vec<f32> = (0..16384)
            .map(|i| (i as f32 / 44100.0 * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5)
            .collect();
        let buf = AudioBuffer::new(44100, samples, 2);
        let result = ffmpeg_encode(&buf, AudioFormat::Flac);
        assert!(result.is_ok(), "FLAC stereo 2-frame encoding failed: {:?}", result);
        assert!(!result.unwrap().is_empty(), "FLAC stereo output is empty");
    }
}
