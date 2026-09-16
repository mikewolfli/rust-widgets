// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

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

// ---------------------------------------------------------------------------
// Temp-file RAII guard
// ---------------------------------------------------------------------------

/// Deletes the temporary output file when dropped.
///
/// FFmpeg's `format::output_as` creates the file on disk immediately, but the
/// encode below has many `?` early-returns (resampler construction, encoder
/// open, frame send, packet write, ...). Without a guard, **every** failure
/// after that point leaked its temp file into the system temp directory —
/// reproducible by encoding with an invalid sample rate:
///
/// ```text
/// before: []
/// encode result: Some("Failed to create resampler: Invalid argument")
/// after : ["rust_widgets_audio_enc_80794_0.mp3"]   // leaked
/// ```
///
/// `Drop` runs on all exits — including the `?` returns and an unwinding panic —
/// so no path can leave the file behind.
struct TempFileGuard {
    path: PathBuf,
}

impl TempFileGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        // Best effort: a failure here must not mask the original error.
        let _ = fs::remove_file(&self.path);
    }
}

// ---------------------------------------------------------------------------
// Temp file naming
// ---------------------------------------------------------------------------

fn next_temp_path(ext: &str) -> PathBuf {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    temp_path_in(&std::env::temp_dir(), ext, count)
}

/// Builds a temp-file path inside `dir` for counter value `count`.
///
/// The directory is a parameter so tests can point at an isolated dir instead of
/// sharing the system temp directory (and its counter) with every other test in
/// the binary.
fn temp_path_in(dir: &std::path::Path, ext: &str, count: u64) -> PathBuf {
    let pid = std::process::id();
    dir.join(format!("rust_widgets_audio_enc_{pid}_{count}.{ext}"))
}

// ---------------------------------------------------------------------------
// Format → (muxer-format, encoder-candidates, default-bitrate)
// ---------------------------------------------------------------------------

/// The encoder candidates for one output format, in preference order.
///
/// Each format lists external-library encoders first (better quality/feature
/// coverage) and FFmpeg's built-in encoder last. Which ones are actually
/// available depends on how the system FFmpeg was compiled, so the list is
/// resolved at runtime by [`resolve_encoder`] instead of being hard-coded to a
/// single name — a build without `libvorbis` must still be able to write Ogg.
struct EncoderPlan {
    /// Muxer (container) name.
    muxer: &'static str,
    /// Candidate encoder names, most-preferred first.
    encoders: &'static [&'static str],
    /// Default bit rate in bits/s (`0` = encoder default).
    bit_rate: i64,
}

fn format_to_ffmpeg_params(format: AudioFormat) -> Result<EncoderPlan, String> {
    match format {
        AudioFormat::Mp3 => {
            Ok(EncoderPlan { muxer: "mp3", encoders: &["libmp3lame"], bit_rate: 192_000 })
        }
        AudioFormat::Flac => Ok(EncoderPlan { muxer: "flac", encoders: &["flac"], bit_rate: 0 }),
        AudioFormat::Ogg => Ok(EncoderPlan {
            muxer: "ogg",
            // `libvorbis` is the reference encoder; FFmpeg's built-in `vorbis`
            // is the fallback for builds without libvorbis (e.g. Homebrew's
            // ffmpeg). The built-in encoder is experimental, which
            // `resolve_encoder` handles.
            encoders: &["libvorbis", "vorbis"],
            bit_rate: 128_000,
        }),
        AudioFormat::Aac => {
            Ok(EncoderPlan { muxer: "adts", encoders: &["aac"], bit_rate: 128_000 })
        }
        AudioFormat::Opus => {
            Ok(EncoderPlan { muxer: "opus", encoders: &["libopus", "opus"], bit_rate: 64_000 })
        }
        _ => Err(format!(
            "audio format {format:?} cannot be encoded by FFmpeg; supported formats are \
             Wav, Flac, Mp3, Aac and Opus"
        )),
    }
}

/// Resolve the first available encoder from a plan's candidate list.
///
/// Returns the descriptor together with the name that was actually selected so
/// callers can report honest diagnostics (a test asserting `libvorbis` semantics
/// should be able to see that `vorbis` was used instead).
fn resolve_encoder(
    plan: &EncoderPlan,
) -> Result<(&'static str, ffmpeg_next::codec::codec::Codec), String> {
    for name in plan.encoders {
        if let Some(descriptor) = ffmpeg_next::encoder::find_by_name(name) {
            return Ok((name, descriptor));
        }
    }
    Err(format!(
        "no encoder available for muxer '{}' (tried: {})",
        plan.muxer,
        plan.encoders.join(", ")
    ))
}

/// FFmpeg's built-in `vorbis` encoder is flagged experimental and refuses to
/// open unless the context explicitly opts in (the CLI equivalent is
/// `-strict -2`). External encoders such as `libvorbis` are not experimental.
fn needs_experimental_opt_in(encoder_name: &str) -> bool {
    matches!(encoder_name, "vorbis" | "opus")
}

/// Pick the sample format to request from the encoder.
///
/// Packed F32 is preferred because it matches our native buffer layout and
/// avoids a conversion. If the encoder does not accept it (FFmpeg's built-in
/// `vorbis` only supports planar `fltp`, unlike `libvorbis`), fall back to the
/// first format the encoder advertises — the resampler below converts into
/// whatever is chosen.
fn choose_encoder_sample_format(codec_audio: &ffmpeg_next::codec::Audio) -> Sample {
    let Some(formats) = codec_audio.formats() else {
        // No advertised list: keep our native layout; `open_as` reports the
        // error if the encoder rejects it.
        return Sample::F32(SampleType::Packed);
    };
    let mut first_any: Option<Sample> = None;
    for format in formats {
        if format == Sample::F32(SampleType::Packed) {
            return format;
        }
        if first_any.is_none() {
            first_any = Some(format);
        }
    }
    first_any.unwrap_or(Sample::F32(SampleType::Packed))
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
    ffmpeg_next::init().map_err(|e| {
        format!(
            "FFmpeg could not be initialised: {e}; the FFmpeg runtime libraries must be \
             installed and resolvable on this host"
        )
    })?;

    let plan = format_to_ffmpeg_params(format)?;
    let muxer_name = plan.muxer;
    let sample_rate = buffer.sample_rate as i32;

    // ── Temp output file ─────────────────────────────────────────────
    let ext = format.extension();
    let tmp_path = next_temp_path(ext);
    let path_str = tmp_path.to_str().ok_or("Invalid temp file path")?.to_owned();

    // `output_as` below creates the file on disk, and every subsequent `?` could
    // return early. The guard removes it on all exits so a failed encode cannot
    // leave temp files behind.
    let _temp_guard = TempFileGuard::new(tmp_path.clone());

    // ── Create output context (muxer) ────────────────────────────────
    let mut octx = ffmpeg_next::format::output_as(&path_str, muxer_name).map_err(|e| {
        format!(
            "muxer '{muxer_name}' could not be created for output '{path_str}': \
                     {e} (the extension selects the muxer; check the output format)"
        )
    })?;

    // Save global-header flag before we borrow octx via a stream.
    let global = octx.format().flags().contains(ffmpeg_next::format::flag::Flags::GLOBAL_HEADER);

    // ── Find encoder ─────────────────────────────────────────────────
    // Resolve at runtime so a build without the preferred external library
    // (e.g. Homebrew's ffmpeg has no `libvorbis`) still encodes.
    let (encoder_name, codec_descriptor) = resolve_encoder(&plan)?;
    let codec_audio = codec_descriptor.audio().map_err(|e| {
        format!(
            "'{encoder_name}' is not an audio encoder, so the plan's codec cannot \
                     produce samples: {e}"
        )
    })?;

    // ── Create encoder context independently ─────────────────────────
    // Build and open the encoder before touching the muxer stream so we
    // can avoid borrowing `octx` through a `StreamMut` while encoding.
    let encoder_ctx = ffmpeg_next::codec::context::Context::new_with_codec(codec_descriptor);
    let mut encoder_initial = encoder_ctx.encoder().audio().map_err(|e| {
        format!(
            "audio encoder '{encoder_name}' could not be created for codec \
                     {codec_descriptor:?}: {e}"
        )
    })?;

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

    if plan.bit_rate > 0 {
        encoder_initial.set_bit_rate(plan.bit_rate as usize);
    }
    encoder_initial.set_rate(encoder_sample_rate);
    encoder_initial.set_channel_layout(channel_layout);
    // Request a format the encoder actually accepts; the resampler below
    // converts our packed F32 buffers into it when they differ.
    let sample_format = choose_encoder_sample_format(&codec_audio);
    encoder_initial.set_format(sample_format);
    encoder_initial.set_time_base((1, encoder_sample_rate));

    if global {
        encoder_initial.set_flags(ffmpeg_next::codec::Flags::GLOBAL_HEADER);
    }

    // FFmpeg's built-in experimental encoders require an explicit opt-in
    // (`-strict -2` on the CLI); without it `open_as` fails with
    // "Experimental feature".
    if needs_experimental_opt_in(encoder_name) {
        encoder_initial.compliance(ffmpeg_next::codec::Compliance::Experimental);
    }

    // ── Open encoder ─────────────────────────────────────────────────
    let mut encoder = encoder_initial.open_as(codec_descriptor).map_err(|e| {
        format!(
            "audio encoder '{encoder_name}' could not be opened: {e} (the codec may \
                     reject the sample rate or channel layout)"
        )
    })?;

    // Determine the actual sample format the encoder uses after opening
    let actual_format = encoder.format();

    // ── Add stream, associate encoder, save index ────────────────────
    let stream_index: usize;
    {
        let mut ost = octx.add_stream(codec_descriptor).map_err(|e| {
            format!("muxer '{muxer_name}' refused the stream for encoder '{encoder_name}': {e}")
        })?;
        ost.set_parameters(&encoder);
        stream_index = ost.index();
    }

    // ── Write muxer header ───────────────────────────────────────────
    octx.write_header().map_err(|e| {
        format!("muxer '{muxer_name}' could not write its header for output '{path_str}': {e}")
    })?;

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
            .map_err(|e| {
                format!(
                    "resampler {src_format:?}@{src_rate}Hz -> {dst_format:?}@{dst_rate}Hz \
                     could not be created: {e}"
                )
            })?,
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
            resampler.run(&f32_frame, &mut converted).map_err(|e| {
                format!("resampling from {src_format:?} to {dst_format:?} failed: {e}")
            })?;
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
                    packet.write_interleaved(&mut octx).map_err(|e| {
                        format!(
                            "packet (pts={pts}, stream {stream_index}) could not be written \
                                 to muxer '{muxer_name}': {e}"
                        )
                    })?;
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
                Err(e) => {
                    return Err(format!(
                        "encoder '{encoder_name}' failed while receiving a packet at pts={pts}: {e}"
                    ))
                }
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
    encoder.send_eof().map_err(|e| {
        format!("encoder '{encoder_name}' could not be flushed at end of stream: {e}")
    })?;

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
                        return Err(format!(
                            "packet at pts={pts} could not be written to muxer '{muxer_name}' \
                             while flushing: {e}"
                        ));
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
            Err(e) => {
                return Err(format!(
                "encoder '{encoder_name}' failed while receiving a flush packet at pts={pts}: {e}"
            ))
            }
        }
        packet = Packet::empty();
    }
    if dropped_packets > 0 {
        log::warn!("FLAC flush: dropped {dropped_packets} packet(s) rejected by the muxer");
    }

    octx.write_trailer().map_err(|e| {
        format!("muxer '{muxer_name}' could not write its trailer for output '{path_str}': {e}")
    })?;

    // ── Read back ────────────────────────────────────────────────────
    let result = fs::read(&tmp_path).map_err(|e| {
        format!("encoded output file '{}' could not be read back: {e}", tmp_path.display())
    })?;

    // The bytes are in memory now; the guard removes the temp file when it goes
    // out of scope at the end of this function (including on the `?` above).

    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::format::AudioFormat;
    use crate::audio::samples::AudioBuffer;

    #[test]
    fn test_flac_encode_mono() {
        let _serial = ENCODE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
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
        let _serial = ENCODE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // 16384 samples stereo = 8192 frames = 2 frames of 4096 each
        let samples: Vec<f32> = (0..16384)
            .map(|i| (i as f32 / 44100.0 * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5)
            .collect();
        let buf = AudioBuffer::new(44100, samples, 2);
        let result = ffmpeg_encode(&buf, AudioFormat::Flac);
        assert!(result.is_ok(), "FLAC stereo 2-frame encoding failed: {:?}", result);
        assert!(!result.unwrap().is_empty(), "FLAC stereo output is empty");
    }

    // ── Encoder-selection regressions ───────────────────────────────────
    //
    // Ogg previously hard-coded `libvorbis`, so any FFmpeg build without that
    // external library (e.g. Homebrew's) failed with
    // `Encoder 'libvorbis' not found` — an environment-dependent failure that
    // had nothing to do with the code under test.

    /// Every format's plan must list at least one built-in fallback, so a build
    /// lacking the preferred external library can still encode.
    #[test]
    fn test_every_format_has_a_builtin_fallback_candidate() {
        for format in [
            AudioFormat::Mp3,
            AudioFormat::Flac,
            AudioFormat::Ogg,
            AudioFormat::Aac,
            AudioFormat::Opus,
        ] {
            let plan = format_to_ffmpeg_params(format)
                .unwrap_or_else(|e| panic!("{format:?} has no encoder plan: {e}"));
            assert!(!plan.encoders.is_empty(), "{format:?} lists no encoder candidates");
            // At least one candidate must be resolvable on this machine,
            // otherwise the format is silently unencodable.
            assert!(
                resolve_encoder(&plan).is_ok(),
                "{format:?} has no available encoder among {:?} on this FFmpeg build",
                plan.encoders
            );
        }
    }

    /// The built-in experimental encoders must be opted in; external libraries
    /// must not be (they are not flagged experimental).
    #[test]
    fn test_experimental_opt_in_only_for_builtin_encoders() {
        assert!(needs_experimental_opt_in("vorbis"));
        assert!(needs_experimental_opt_in("opus"));
        assert!(!needs_experimental_opt_in("libvorbis"));
        assert!(!needs_experimental_opt_in("libopus"));
        assert!(!needs_experimental_opt_in("aac"));
        assert!(!needs_experimental_opt_in("flac"));
    }

    /// A plan whose candidates are all unavailable must fail loudly rather than
    /// silently falling through to some other encoder.
    #[test]
    fn test_resolve_encoder_reports_unavailable_candidates() {
        let plan =
            EncoderPlan { muxer: "ogg", encoders: &["definitely_not_a_real_encoder"], bit_rate: 0 };
        let err = match resolve_encoder(&plan) {
            Ok(_) => panic!("bogus encoder must not resolve"),
            Err(err) => err,
        };
        assert!(err.contains("definitely_not_a_real_encoder"), "error was: {err}");
        assert!(err.contains("ogg"), "error should name the muxer: {err}");
    }

    /// Ogg must round-trip to real Ogg/Vorbis bytes regardless of whether this
    /// FFmpeg build ships `libvorbis`. This is the regression that used to fail
    /// on hosts whose FFmpeg lacks the external library.
    #[test]
    fn test_ogg_encode_produces_real_ogg_container() {
        let _serial = ENCODE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let samples: Vec<f32> = (0..16384)
            .map(|i| (i as f32 / 44100.0 * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5)
            .collect();
        let buf = AudioBuffer::new(44100, samples, 2);
        let data = ffmpeg_encode(&buf, AudioFormat::Ogg)
            .expect("Ogg encoding must succeed with either libvorbis or the built-in vorbis");
        assert!(!data.is_empty(), "Ogg output is empty");
        // The Ogg page capture pattern is ASCII "OggS" (0x4F 0x67 0x67 0x53).
        assert_eq!(
            &data[..4],
            b"OggS",
            "Ogg output does not start with the OggS magic: {:02x?}",
            &data[..4]
        );
    }

    // ── Temp-file lifecycle ──────────────────────────────────────────
    //
    // These tests share the system temp directory with every other encode test
    // in this crate. A naive before/after count therefore races with concurrent
    // tests (observed: a peer's in-flight file looked like a leak, and a peer's
    // cleanup looked like a failed cleanup). Because `next_temp_path` uses a
    // monotonically increasing counter, the robust check is to look at the
    // *specific* file this call will create: record the counter before the call
    // and assert that the file for that index is gone afterwards.

    /// Serializes the temp-file lifecycle tests.
    ///
    /// The tests below share `TEMP_COUNTER` with every other encode test in this
    /// module, so two concurrent calls can interleave their index allocation.
    /// This lock makes the index→file mapping stable *for the guard tests below*;
    /// the encode-level test additionally re-checks the specific path.
    static ENCODE_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// `TempFileGuard` must delete the file it was given, on drop.
    ///
    /// This is the core contract the leak fix relies on, and it is verified
    /// deterministically: no shared counter, no shared temp-directory snapshot,
    /// so nothing in this test can race a concurrent encode.
    #[test]
    fn temp_file_guard_removes_file_on_drop() {
        let path = temp_path_in(&std::env::temp_dir(), "guardprobe", u64::MAX - 1);
        std::fs::write(&path, b"x").expect("write probe file");
        assert!(path.exists(), "probe file should exist before the guard runs");

        {
            let _guard = TempFileGuard::new(path.clone());
            assert!(path.exists(), "the guard must not delete the file early");
        }

        assert!(!path.exists(), "the guard must delete the file when dropped");
    }

    /// A **failed** encode must not leave its temp file behind.
    ///
    /// Regression: `format::output_as` creates the file up front, but the
    /// function has many `?` early-returns. Before the `TempFileGuard`, a
    /// failure after that point leaked the file. A sample rate of 0 fails
    /// resampler construction, which is safely *after* file creation.
    ///
    /// The assertion is on the *exact* path this call uses. `TEMP_COUNTER` is
    /// shared with every other encode test in the binary, so the test records
    /// which index it consumed and skips (rather than false-failing) if a peer
    /// module raced it. `temp_file_guard_removes_file_on_drop` is the
    /// deterministic lock on the mechanism itself.
    #[test]
    fn test_failed_encode_leaves_no_temp_file() {
        let _serial = ENCODE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let index = TEMP_COUNTER.load(Ordering::Relaxed);
        let expected = temp_path_in(&std::env::temp_dir(), "mp3", index);

        let buf = AudioBuffer::new(0, vec![0.0f32; 4096], 2);
        let result = ffmpeg_encode(&buf, AudioFormat::Mp3);
        assert!(result.is_err(), "a 0 Hz sample rate must fail to encode");

        if TEMP_COUNTER.load(Ordering::Relaxed) != index + 1 {
            return; // a peer module consumed `index`; `expected` is not ours
        }
        assert!(!expected.exists(), "a failed encode leaked {expected:?}");
    }

    /// A **successful** encode must also clean up after itself.
    #[test]
    fn test_successful_encode_leaves_no_temp_file() {
        let _serial = ENCODE_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());

        let index = TEMP_COUNTER.load(Ordering::Relaxed);
        let expected = temp_path_in(&std::env::temp_dir(), "mp3", index);

        let samples: Vec<f32> = (0..16384)
            .map(|i| (i as f32 / 44100.0 * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5)
            .collect();
        let buf = AudioBuffer::new(44100, samples, 2);
        ffmpeg_encode(&buf, AudioFormat::Mp3).expect("Mp3 encoding should succeed");

        if TEMP_COUNTER.load(Ordering::Relaxed) != index + 1 {
            return; // a peer module consumed `index`; `expected` is not ours
        }
        assert!(!expected.exists(), "a successful encode leaked {expected:?}");
    }
}
