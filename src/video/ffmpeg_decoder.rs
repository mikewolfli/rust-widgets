// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Real video decoder powered by FFmpeg (via ffmpeg-next).
//! Gated behind `#[cfg(feature = "video-codecs")]` which provides
//! hardware-accelerated decoding for MP4, AVI, MKV, WebM, FLV, WMV, MOV
//! and many other container formats.
//!
//! Decodes frames to RGBA8 format and populates full `VideoMetadata`.

use std::collections::VecDeque;
use std::io::Cursor;

use ffmpeg_next::codec::decoder::Video as VideoDecoder;
use ffmpeg_next::format::context::Input;
use ffmpeg_next::format::context::StreamIo;
use ffmpeg_next::media;
use ffmpeg_next::software;
use ffmpeg_next::util::format;
use ffmpeg_next::Rational;

use crate::video::format::ContainerFormat;
use crate::video::frame::{FrameType, VideoFrame};
use crate::video::metadata::VideoMetadata;
use crate::video::VideoDecoder as VideoDecoderTrait;

// ---------------------------------------------------------------------------
// Standalone convenience: decode entire video in one shot
// ---------------------------------------------------------------------------

/// Decode a complete video file from raw bytes using FFmpeg.
///
/// Returns all decoded RGBA8 frames and the video metadata.
/// Supports any container format that FFmpeg can demux.
///
/// # Errors
///
/// Returns a human-readable error string if FFmpeg initialisation,
/// demuxing, or decoding fails.
pub fn decode_frames(data: &[u8]) -> Result<(Vec<VideoFrame>, VideoMetadata), String> {
    let mut decoder = FfmpegDecoder::new(data.to_vec())?;
    let metadata = decoder.metadata().clone();
    // `total_frames` is derived from the container's duration and frame rate, so a malformed or
    // hostile header could name millions of frames. Reserving that count up front turned a bad
    // header into a huge allocation before a single frame was decoded. Reserve a bounded working
    // set instead and let the vector grow with the frames that really arrive.
    const FRAME_PREALLOC_CAP: usize = 64;
    let mut frames = Vec::with_capacity((metadata.total_frames as usize).min(FRAME_PREALLOC_CAP));

    loop {
        match decoder.read_frame() {
            Ok(Some(frame)) => frames.push(frame),
            Ok(None) => break,
            Err(e) => return Err(e),
        }
    }

    Ok((frames, metadata))
}

// ---------------------------------------------------------------------------
// FfmpegDecoder — streaming decoder implementing the VideoDecoder trait
// ---------------------------------------------------------------------------

/// Streaming FFmpeg-based video decoder.
///
/// Opens a video from raw bytes, uses FFmpeg to demux and decode, and
/// converts every frame to RGBA8 on the fly.  Implements the project's
/// `VideoDecoder` trait so it can be used anywhere a `Box<dyn VideoDecoder>`
/// is expected.
pub struct FfmpegDecoder {
    /// FFmpeg demuxer context (owns all stream information).
    input: Input,
    /// Opened video decoder (owns codec context).
    decoder: VideoDecoder,
    /// Software scaler converting decoder output → RGBA.
    scaler: software::scaling::Context,
    /// Stream metadata extracted during construction.
    metadata: VideoMetadata,
    /// Index of the video stream we are decoding.
    stream_index: usize,
    /// Time base of the video stream (for PTS → seconds conversion).
    time_base: Rational,

    // ── streaming state ──────────────────────────────────────────────
    /// Whether the demuxer has been fully consumed.
    eof: bool,
    /// True after `send_eof()` has been called (decoder flushed).
    flushed: bool,
    /// Frames decoded from the current / previous packets but not yet
    /// returned by `read_frame()`.
    buffered: VecDeque<VideoFrame>,
    /// Running frame counter for synthesising timestamps when PTS is
    /// unavailable.
    frame_index: u64,
}

// SAFETY: All internal FFmpeg pointers (`AVFormatContext`, `AVCodecContext`,
// `SwsContext`) are not tied to any particular OS thread.  The scaler
// (`SwsContext`) is reentrant and safe to move between threads as long as
// it is not used concurrently on multiple threads, which our API guarantees.
unsafe impl Send for FfmpegDecoder {}

impl FfmpegDecoder {
    /// Create a new FFmpeg decoder from raw video bytes.
    ///
    /// # Why the bytes are not written to a temporary file
    ///
    /// The first implementation wrote the whole buffer to a temp path and asked FFmpeg to open
    /// it, which cost a full disk write plus a read-back on the calling thread for a source the
    /// caller already had in memory (MEDIA-03). ffmpeg-next 9 exposes custom AVIO through
    /// [`StreamIo`], so the demuxer reads a seekable [`Cursor`] over the caller's bytes instead:
    /// no file is created, and nothing needs cleaning up on success or on an error path.
    ///
    /// The stream stays seekable, so [`VideoDecoderTrait::seek`] keeps working exactly as it did
    /// over a file.
    pub fn new(data: Vec<u8>) -> Result<Self, String> {
        ffmpeg_next::init().map_err(|e| {
            format!(
                "FFmpeg could not be initialised: {e}; the FFmpeg runtime libraries must be \
                 installed and resolvable on this host"
            )
        })?;

        let byte_len = data.len();
        let stream_io = StreamIo::from_read_seek(Cursor::new(data)).map_err(|e| {
            format!("{byte_len} in-memory bytes could not be wrapped for FFmpeg's demuxer: {e}")
        })?;
        let input = ffmpeg_next::format::input_from_stream(stream_io, None, None).map_err(|e| {
            format!(
                "FFmpeg could not open the in-memory source ({byte_len} bytes; unsupported or \
                 corrupt container): {e}"
            )
        })?;

        Self::from_input(input, "in-memory source")
    }

    /// Build a decoder over an already-open demuxer.
    ///
    /// `label` names the source in diagnostics; it used to be a path that both the open and the
    /// "no video stream" message interpolated.
    fn from_input(input: Input, label: &str) -> Result<Self, String> {
        // Find the best video stream.
        let stream = input.streams().best(media::Type::Video).ok_or_else(|| {
            format!(
                "input '{label}' has no video stream ({} stream(s) present): audio-only \
                 containers cannot be decoded into frames",
                input.streams().len()
            )
        })?;

        let stream_index = stream.index();
        let time_base = stream.time_base();

        // Build codec parameters → decoder context.
        let codec_ctx =
            ffmpeg_next::codec::Context::from_parameters(stream.parameters()).map_err(|e| {
                format!(
                    "video codec parameters could not be turned into a decoder context: {e} \
                     (the stream header may be truncated)"
                )
            })?;

        // `width()`/`height()` moved from `codec::Context` to the *opened decoder* in
        // ffmpeg-next 9.0, so asking the context first is what broke this build.
        // The decoder is not open yet at this point, so the dimensions come from the
        // codec parameters the stream already carries.
        let (source_width, source_height) = {
            let params = stream.parameters();
            // SAFETY: `params` points at the codec parameters owned by the live `stream`; only two fields are read.
            unsafe { ((*params.as_ptr()).width, (*params.as_ptr()).height) }
        };
        let decoder = codec_ctx.decoder().video().map_err(|e| {
            format!("video decoder {source_width}x{source_height} could not be opened: {e}")
        })?;

        // Create the RGBA scaler.
        let scaler = software::converter(
            (decoder.width(), decoder.height()),
            decoder.format(),
            format::Pixel::RGBA,
        )
        .map_err(|e| {
            format!(
                "colour converter {}x{} {:?} -> RGBA could not be created: {e}",
                decoder.width(),
                decoder.height(),
                decoder.format()
            )
        })?;

        let metadata = build_metadata(&input, &decoder, &stream);

        Ok(Self {
            input,
            decoder,
            scaler,
            metadata,
            stream_index,
            time_base,
            eof: false,
            flushed: false,
            buffered: VecDeque::new(),
            frame_index: 0,
        })
    }

    /// Convert a decoded FFmpeg video frame to RGBA8 and wrap it in a
    /// `VideoFrame`.
    fn convert_frame(&mut self, frame: &ffmpeg_next::frame::Video) -> Result<VideoFrame, String> {
        let width = frame.width();
        let height = frame.height();
        let mut rgb = ffmpeg_next::frame::Video::empty();

        self.scaler.run(frame, &mut rgb).map_err(|e| {
            format!(
                "frame {width}x{height} could not be converted to RGBA for output: {e} \
                     (the scaler needs matching source format and size)"
            )
        })?;

        // The scaler has now allocated the output frame; read its data.
        // `rgb.data(0)` is a raw byte slice whose rows may be padded to an
        // arbitrary stride (the scaler is free to align each row), so a plain
        // `to_vec()` would hand the consumer a buffer that is wider than
        // `width * 4` per row and whose pixels are silently misaligned. Pack it
        // row by row into the tight `width * height * 4` RGBA layout the rest of
        // the pipeline (and `VideoFrame`) expects (N-S-54).
        let data = pack_rgba_rows(rgb.data(0), rgb.stride(0), width as usize, height as usize)
            .map_err(|e| format!("frame {width}x{height} could not be packed for output: {e}"))?;

        // Determine timestamp.
        let pts = frame.pts();
        let ts = match pts {
            Some(pts) => {
                pts as f64 * self.time_base.numerator() as f64 / self.time_base.denominator() as f64
            }
            None => self.frame_index as f64 / self.metadata.frame_rate.max(1.0),
        };

        let frame_type = if frame.is_key() { FrameType::IFrame } else { FrameType::PFrame };

        Ok(VideoFrame::with_type(ts, data, width, height, frame_type))
    }

    /// Read the next packet from the demuxer that belongs to our video
    /// stream, or `None` on EOF.
    fn read_packet(&mut self) -> Result<Option<ffmpeg_next::Packet>, String> {
        if self.eof {
            return Ok(None);
        }

        loop {
            let mut packet = ffmpeg_next::Packet::empty();
            match packet.read(&mut self.input) {
                Ok(()) => {
                    if packet.stream() == self.stream_index {
                        return Ok(Some(packet));
                    }
                    // Skip non-video streams.
                }
                Err(ffmpeg_next::Error::Eof) => {
                    self.eof = true;
                    return Ok(None);
                }
                Err(e) => {
                    return Err(format!(
                        "packet after frame {} could not be read from the container (the file \
                         may be truncated): {e}",
                        self.frame_index
                    ))
                }
            }
        }
    }

    /// Decode all frames that the decoder can produce from its current
    /// internal buffer (after a `send_packet` or `send_eof`).
    fn drain_decoder(&mut self) -> Result<(), String> {
        loop {
            let mut frame = ffmpeg_next::frame::Video::empty();
            match self.decoder.receive_frame(&mut frame) {
                Ok(()) => {
                    self.frame_index += 1;
                    match self.convert_frame(&frame) {
                        Ok(vf) => self.buffered.push_back(vf),
                        Err(e) => log::warn!(
                            "[FfmpegDecoder] frame {} was skipped because it could not be \
                             converted to RGBA: {e}",
                            self.frame_index
                        ),
                    }
                }
                Err(ffmpeg_next::Error::Eof) => break,
                _ => {
                    // EAGAIN or other transient / expected states.
                    break;
                }
            }
        }
        Ok(())
    }
}

impl VideoDecoderTrait for FfmpegDecoder {
    fn read_frame(&mut self) -> Result<Option<VideoFrame>, String> {
        // 1. Return buffered frames first.
        if let Some(frame) = self.buffered.pop_front() {
            return Ok(Some(frame));
        }

        // 2. Already fully consumed and flushed → nothing left.
        if self.eof && self.flushed {
            return Ok(None);
        }

        // 3. Flush remaining frames from the decoder if demuxer is done.
        if self.eof && !self.flushed {
            self.decoder.send_eof().map_err(|e| format!("Failed to send EOF to decoder: {e}"))?;
            self.flushed = true;
            self.drain_decoder()?;
            return Ok(self.buffered.pop_front());
        }

        // 4. Normal operation: read packets, send to decoder, collect frames.
        while let Some(packet) = self.read_packet()? {
            self.decoder
                .send_packet(&packet)
                .map_err(|e| format!("Failed to send packet to decoder: {e}"))?;
            self.drain_decoder()?;

            if let Some(frame) = self.buffered.pop_front() {
                return Ok(Some(frame));
            }
            // The packet may not have produced a frame yet; keep reading.
        }

        // 5. Demuxer EOF — flush decoder.
        self.decoder.send_eof().map_err(|e| format!("Failed to send EOF to decoder: {e}"))?;
        self.flushed = true;
        self.drain_decoder()?;

        Ok(self.buffered.pop_front())
    }

    fn seek(&mut self, time: f64) -> Result<(), String> {
        // `Input::seek` forwards to `avformat_seek_file`, whose timestamp is in
        // **global AV_TIME_BASE microseconds** (stream_index = -1), not in the
        // stream's own time base. Converting to stream ticks and passing those as
        // if they were AV_TIME_BASE units sought to a wildly wrong place — the
        // old `time * den / num` produced a value roughly `time_base` times too
        // large (N-S-53).
        let target = seconds_to_av_time_base(time)?;

        self.input.seek(target, ..).map_err(|e| {
            format!(
                "seek to {time:.3}s (AV_TIME_BASE {target}) failed: {e}; the container may \
                 not be seekable"
            )
        })?;

        // Flush decoder buffers so the next frame decode starts fresh.
        self.decoder.flush();
        self.buffered.clear();
        self.eof = false;
        self.flushed = false;
        Ok(())
    }

    fn close(&mut self) -> Result<(), String> {
        self.buffered.clear();
        self.eof = true;
        self.flushed = true;
        Ok(())
    }

    fn metadata(&self) -> &VideoMetadata {
        &self.metadata
    }
}

impl Drop for FfmpegDecoder {
    fn drop(&mut self) {
        // The demuxer owns its custom IO (a `Cursor` over the caller's bytes), so dropping the
        // input releases the buffer; there is no file to unlink (MEDIA-03).
        self.buffered.clear();
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// FFmpeg's global time base: one second is `AV_TIME_BASE` units, the unit
/// `avformat_seek_file` expects when seeking by timestamp rather than by frame.
const AV_TIME_BASE: i64 = 1_000_000;

/// Converts a seek target in seconds to `AV_TIME_BASE` microseconds.
///
/// Rejects a non-finite or out-of-range time (negative, or so large the product
/// cannot be represented) instead of casting a saturating/`NaN` `f64` to a
/// timestamp and seeking to an arbitrary place (N-S-53).
fn seconds_to_av_time_base(seconds: f64) -> Result<i64, String> {
    if !seconds.is_finite() {
        return Err(format!("seek target {seconds} is not a finite number of seconds"));
    }
    if seconds < 0.0 {
        return Err(format!(
            "seek target {seconds}s is negative; seeks are measured from the start"
        ));
    }
    let micros = seconds * AV_TIME_BASE as f64;
    if micros > i64::MAX as f64 {
        return Err(format!(
            "seek target {seconds}s is too large to represent in AV_TIME_BASE microseconds"
        ));
    }
    Ok(micros as i64)
}

/// Packs a strided RGBA plane into a tight `width * height * 4` buffer.
///
/// `plane` is the raw first-plane data and `stride` the number of bytes between
/// the start of consecutive rows. A row is copied as exactly `width * 4` bytes
/// from its start, so any per-row padding is dropped and the result is the tight
/// layout every consumer expects. Returns an explicit error when the plane is
/// too short to hold the rows it claims (N-S-54).
fn pack_rgba_rows(
    plane: &[u8],
    stride: usize,
    width: usize,
    height: usize,
) -> Result<Vec<u8>, String> {
    let row_bytes =
        width.checked_mul(4).ok_or_else(|| format!("RGBA row width {width} × 4 overflows"))?;
    let total = row_bytes
        .checked_mul(height)
        .ok_or_else(|| format!("RGBA buffer {width}x{height} overflows"))?;
    if height > 0 && stride < row_bytes {
        return Err(format!(
            "RGBA plane stride {stride} is smaller than one row of {row_bytes} bytes"
        ));
    }
    // Only the rows that actually exist need to fit: a zero-height plane has no
    // rows and is trivially valid even though it carries no bytes.
    if height > 0 {
        let last_row_start = stride
            .checked_mul(height - 1)
            .ok_or_else(|| "RGBA plane row offset overflows".to_string())?;
        let last_row_end = last_row_start
            .checked_add(row_bytes)
            .ok_or_else(|| "RGBA plane end offset overflows".to_string())?;
        if last_row_end > plane.len() {
            return Err(format!(
                "RGBA plane is {} bytes but {width}x{height} at stride {stride} needs {last_row_end}",
                plane.len()
            ));
        }
    }
    let mut out = Vec::with_capacity(total);
    for row in 0..height {
        let start = row * stride;
        out.extend_from_slice(&plane[start..start + row_bytes]);
    }
    Ok(out)
}

/// Build a complete `VideoMetadata` from the demuxer, decoder, and stream.
fn build_metadata(
    input: &Input,
    decoder: &VideoDecoder,
    stream: &ffmpeg_next::Stream<'_>,
) -> VideoMetadata {
    let container = detect_container_from_ffmpeg(input);
    let codec_name = codec_name_from_id(decoder.id());
    let width = decoder.width();
    let height = decoder.height();
    let duration_secs = duration_in_seconds(input);
    let bitrate = input.bit_rate().max(0) as u64;
    let (frame_rate, total_frames) = frame_rate_and_total(stream, decoder);

    let has_audio = input.streams().best(media::Type::Audio).is_some();

    let audio_codec = if has_audio {
        if let Some(audio_stream) = input.streams().best(media::Type::Audio) {
            let id = audio_stream.parameters().id();
            codec_name_from_id(id)
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    VideoMetadata {
        container,
        duration: duration_secs,
        codec: codec_name,
        width,
        height,
        frame_rate,
        bitrate,
        has_audio,
        audio_codec,
        total_frames,
    }
}

/// Extract duration from the format context in seconds.
fn duration_in_seconds(input: &Input) -> f64 {
    const AV_TIME_BASE: i64 = 1_000_000;
    let dur = input.duration();
    if dur > 0 {
        dur as f64 / AV_TIME_BASE as f64
    } else {
        0.0
    }
}

/// Best-effort frame rate and total frame count.
fn frame_rate_and_total(stream: &ffmpeg_next::Stream<'_>, decoder: &VideoDecoder) -> (f64, u64) {
    // Prefer average frame rate from stream, then r_frame_rate, then
    // fall back to the codec context's framerate.
    let avg = stream.avg_frame_rate();
    let rate = stream.rate();
    let dec_rate = decoder.frame_rate();

    let rational = if avg.numerator() > 0 && avg.denominator() > 0 {
        avg
    } else if rate.numerator() > 0 && rate.denominator() > 0 {
        rate
    } else if let Some(r) = dec_rate {
        r
    } else {
        Rational::new(30, 1)
    };

    let fps = rational.numerator() as f64 / rational.denominator() as f64;

    // Number of frames from stream metadata (may be 0 / unknown).
    let stream_frames = stream.frames();
    let total = if stream_frames > 0 {
        // A known frame count is authoritative; the duration-based estimate is
        // only a fallback for containers that do not report one.
        stream_frames as u64
    } else if fps > 0.0 {
        total_frames_from_duration(stream.duration(), stream.time_base(), fps)
    } else {
        0
    };

    (fps, total)
}

/// Estimates the total frame count from a stream duration, its time base, and
/// the frame rate.
///
/// `duration` is in the stream's own time base units, so the elapsed seconds are
/// `duration × tb.numerator / tb.denominator`; multiplying by `fps` gives frames.
/// The time base's numerator must be included — for a typical MPEG stream the
/// time base is `1/90000`, and omitting the numerator under-counts by a factor
/// of the denominator (N-S-56). A non-positive denominator or duration yields 0
/// rather than dividing by zero.
fn total_frames_from_duration(duration: i64, time_base: Rational, fps: f64) -> u64 {
    if duration <= 0 || !fps.is_finite() || fps <= 0.0 {
        return 0;
    }
    let tb_num = time_base.numerator() as f64;
    let tb_den = time_base.denominator() as f64;
    if tb_den == 0.0 {
        return 0;
    }
    let seconds = duration as f64 * tb_num / tb_den;
    let frames = (seconds * fps).round();
    if frames <= 0.0 {
        0
    } else {
        frames as u64
    }
}

/// Map an FFmpeg codec Id to a human-readable name.
fn codec_name_from_id(id: ffmpeg_next::codec::Id) -> String {
    match id {
        ffmpeg_next::codec::Id::H264 => "h264".into(),
        ffmpeg_next::codec::Id::HEVC => "hevc".into(),
        ffmpeg_next::codec::Id::VP9 => "vp9".into(),
        ffmpeg_next::codec::Id::VP8 => "vp8".into(),
        ffmpeg_next::codec::Id::AV1 => "av1".into(),
        ffmpeg_next::codec::Id::MPEG4 => "mpeg4".into(),
        ffmpeg_next::codec::Id::MPEG2VIDEO => "mpeg2video".into(),
        ffmpeg_next::codec::Id::MJPEG => "mjpeg".into(),
        ffmpeg_next::codec::Id::H261 => "h261".into(),
        ffmpeg_next::codec::Id::H263 => "h263".into(),
        ffmpeg_next::codec::Id::RV10 => "rv10".into(),
        ffmpeg_next::codec::Id::RV20 => "rv20".into(),
        ffmpeg_next::codec::Id::MSMPEG4V1 => "msmpeg4v1".into(),
        ffmpeg_next::codec::Id::MSMPEG4V2 => "msmpeg4v2".into(),
        ffmpeg_next::codec::Id::MSMPEG4V3 => "msmpeg4v3".into(),
        ffmpeg_next::codec::Id::WMV1 => "wmv1".into(),
        ffmpeg_next::codec::Id::WMV2 => "wmv2".into(),
        ffmpeg_next::codec::Id::WMV3 => "wmv3".into(),
        ffmpeg_next::codec::Id::VC1 => "vc1".into(),
        ffmpeg_next::codec::Id::INDEO3 => "indeo3".into(),
        ffmpeg_next::codec::Id::INDEO4 => "indeo4".into(),
        ffmpeg_next::codec::Id::INDEO5 => "indeo5".into(),
        ffmpeg_next::codec::Id::FLV1 => "flv1".into(),
        ffmpeg_next::codec::Id::TSCC => "tscc".into(),
        ffmpeg_next::codec::Id::RAWVIDEO => "rawvideo".into(),
        ffmpeg_next::codec::Id::PNG => "png".into(),
        ffmpeg_next::codec::Id::APNG => "apng".into(),
        ffmpeg_next::codec::Id::DVVIDEO => "dvvideo".into(),
        ffmpeg_next::codec::Id::DNXHD => "dnxhd".into(),
        ffmpeg_next::codec::Id::THEORA => "theora".into(),
        ffmpeg_next::codec::Id::FFV1 => "ffv1".into(),

        ffmpeg_next::codec::Id::SMC => "smc".into(),
        ffmpeg_next::codec::Id::R210 => "r210".into(),
        ffmpeg_next::codec::Id::V210 => "v210".into(),
        _ => {
            let s = format!("{:?}", id);
            s.to_lowercase()
        }
    }
}

/// Detect the project's `ContainerFormat` from the FFmpeg demuxer name.
fn detect_container_from_ffmpeg(input: &Input) -> ContainerFormat {
    let fmt = input.format();
    let name = fmt.name();
    match name {
        "mp4" | "mov,mp4,m4a,3gp,3g2,mj2" => ContainerFormat::Mp4,
        "avi" => ContainerFormat::Avi,
        "matroska" | "matroska,webm" => ContainerFormat::Mkv,
        "webm" => ContainerFormat::WebM,
        "flv" => ContainerFormat::Flv,
        "wmv" | "asf" => ContainerFormat::Wmv,
        "mov" | "quicktime" => ContainerFormat::Mov,
        "mjpeg" => ContainerFormat::Mjpeg,
        _ => ContainerFormat::Unknown,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal valid MP4 file for testing.
    /// This is an ftyp-only stub; it will fail at demuxing but that's OK
    /// for testing the error path.
    fn small_mp4_data() -> Vec<u8> {
        let mut data = Vec::new();
        // ftyp box
        let ftyp_size: u32 = 24u32.to_be();
        data.extend_from_slice(&ftyp_size.to_be_bytes());
        data.extend_from_slice(b"ftyp");
        data.extend_from_slice(b"mp42");
        data.extend_from_slice(&[0u8; 4]); // minor_version
        data.extend_from_slice(b"mp42"); // compatible brand
        data.extend_from_slice(b"isom"); // compatible brand
        data
    }

    #[test]
    fn test_ffmpeg_init() {
        assert!(ffmpeg_next::init().is_ok());
    }

    #[test]
    fn test_decode_invalid_data() {
        let _serial = DECODER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let result = FfmpegDecoder::new(vec![0u8; 100]);
        assert!(result.is_err(), "expected error for invalid video data");
    }

    #[test]
    fn test_decode_invalid_mp4() {
        let _serial = DECODER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // An ftyp box with no moov → should fail gracefully.
        let data = small_mp4_data();
        let result = FfmpegDecoder::new(data);
        assert!(result.is_err(), "expected error for header-only MP4");
    }

    #[test]
    fn test_decode_empty() {
        let _serial = DECODER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let result = FfmpegDecoder::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_standalone_decode_frames_empty() {
        let _serial = DECODER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let result = decode_frames(b"");
        assert!(result.is_err());
    }

    #[test]
    fn test_standalone_decode_frames_invalid() {
        let _serial = DECODER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let result = decode_frames(&[0u8; 256]);
        assert!(result.is_err());
    }

    #[test]
    fn test_codec_name_known() {
        assert_eq!(codec_name_from_id(ffmpeg_next::codec::Id::H264), "h264");
        assert_eq!(codec_name_from_id(ffmpeg_next::codec::Id::VP9), "vp9");
        assert_eq!(codec_name_from_id(ffmpeg_next::codec::Id::MJPEG), "mjpeg");
    }

    /// Returns true when this process has any decoder temp file left in the temp directory.
    ///
    /// The decoder no longer creates one (MEDIA-03) — it reads the caller's bytes through a
    /// seekable in-memory AVIO stream. This scans for the prefix the old implementation used so a
    /// regression that starts writing files again is caught rather than silently accepted.
    fn any_decoder_temp_file_exists() -> bool {
        let pid = std::process::id();
        let prefix = format!("rust_widgets_ffmpeg_{pid}_");
        std::fs::read_dir(std::env::temp_dir())
            .map(|entries| {
                entries
                    .filter_map(|e| e.ok())
                    .any(|e| e.file_name().to_string_lossy().starts_with(&prefix))
            })
            .unwrap_or(false)
    }

    /// Serializes the tests that assert on process-global temp-directory state.
    static DECODER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// A **failed** decoder construction must not write a temp file at all.
    ///
    /// This is the regression gate for MEDIA-03: the old implementation wrote the whole buffer
    /// to disk before asking FFmpeg to open it, so a failed decode could leave the file behind.
    #[test]
    fn test_failed_decoder_leaves_no_temp_file() {
        let _serial = DECODER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let result = FfmpegDecoder::new(vec![0u8; 100]);
        assert!(result.is_err(), "invalid video data must fail to decode");
        assert!(!any_decoder_temp_file_exists(), "a failed decoder construction wrote a temp file");
    }

    /// Construction must not write a temp file, whether it succeeds or fails.
    #[test]
    fn test_decoder_leaves_no_temp_file() {
        let _serial = DECODER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let _ = FfmpegDecoder::new(small_mp4_data());
        assert!(!any_decoder_temp_file_exists(), "a decoder temp file survived construction");
    }

    /// N-S-53: the seek target must be expressed in AV_TIME_BASE microseconds,
    /// with non-finite/negative/oversized inputs rejected rather than cast.
    #[test]
    fn test_seconds_to_av_time_base_conversion() {
        assert_eq!(seconds_to_av_time_base(0.0).unwrap(), 0);
        assert_eq!(seconds_to_av_time_base(1.0).unwrap(), 1_000_000);
        assert_eq!(seconds_to_av_time_base(2.5).unwrap(), 2_500_000);
        // A stream time base must never be used here: 1/90000 would be 90000×
        // too large. Sanity: a whole second is exactly AV_TIME_BASE.
        assert_eq!(seconds_to_av_time_base(1.0).unwrap(), AV_TIME_BASE);

        assert!(seconds_to_av_time_base(-1.0).is_err(), "negative seek is rejected");
        assert!(seconds_to_av_time_base(f64::NAN).is_err(), "NaN is rejected");
        assert!(seconds_to_av_time_base(f64::INFINITY).is_err(), "infinity is rejected");
        assert!(seconds_to_av_time_base(1e300).is_err(), "an unrepresentable time is rejected");
    }

    /// N-S-54: `pack_rgba_rows` must drop per-row padding and always produce a
    /// tight `width * height * 4` buffer.
    #[test]
    fn test_pack_rgba_rows_drops_padding() {
        // 2x2 RGBA with a row stride of 12 bytes (4 bytes of padding per row).
        let stride = 12usize;
        let mut plane = Vec::new();
        // Row 0: red, green, then 4 padding bytes.
        plane.extend_from_slice(&[255, 0, 0, 255, 0, 255, 0, 255, 0xAA, 0xBB, 0xCC, 0xDD]);
        // Row 1: blue, white, then 4 padding bytes.
        plane.extend_from_slice(&[0, 0, 255, 255, 255, 255, 255, 255, 0x11, 0x22, 0x33, 0x44]);

        let packed = pack_rgba_rows(&plane, stride, 2, 2).expect("packs");
        assert_eq!(packed.len(), 2 * 2 * 4, "output must be tight");
        assert_eq!(&packed[0..8], &[255, 0, 0, 255, 0, 255, 0, 255], "row 0 kept, padding dropped");
        assert_eq!(
            &packed[8..16],
            &[0, 0, 255, 255, 255, 255, 255, 255],
            "row 1 kept, padding dropped"
        );
    }

    /// A stride equal to the row size (already tight) is a copy; a plane that is
    /// too short for its rows is an explicit error, not a panic.
    #[test]
    fn test_pack_rgba_rows_tight_and_error_cases() {
        let tight = vec![1u8, 2, 3, 4, 5, 6, 7, 8];
        assert_eq!(pack_rgba_rows(&tight, 8, 2, 1).unwrap(), tight);

        // Height 2, stride 8, row 8 bytes → needs 16 bytes, only 8 given.
        assert!(pack_rgba_rows(&tight, 8, 2, 2).is_err());
        // A stride smaller than a row cannot describe the image.
        assert!(pack_rgba_rows(&tight, 2, 2, 1).is_err());
        // Zero-height is trivially empty and valid.
        assert_eq!(pack_rgba_rows(&[], 8, 2, 0).unwrap().len(), 0);
    }

    /// N-S-56: the duration→frames estimate must include the time base numerator.
    #[test]
    fn test_total_frames_from_duration_includes_time_base_numerator() {
        // 90000 ticks at 1/90000 = exactly 1 second; at 30 fps = 30 frames.
        assert_eq!(total_frames_from_duration(90000, Rational::new(1, 90000), 30.0), 30);
        // A rational time base with a numerator other than 1: 2/1000 = 2 ms per
        // tick, so 1000 ticks = 2 seconds → 60 frames at 30 fps. Omitting the
        // numerator would give 30.
        assert_eq!(total_frames_from_duration(1000, Rational::new(2, 1000), 30.0), 60);
        // 1/1 base: 5 ticks = 5 seconds → 150 frames at 30 fps.
        assert_eq!(total_frames_from_duration(5, Rational::new(1, 1), 30.0), 150);
        // Degenerate inputs return 0 rather than dividing by zero or saturating.
        assert_eq!(total_frames_from_duration(0, Rational::new(1, 30), 30.0), 0);
        assert_eq!(total_frames_from_duration(10, Rational::new(1, 0), 30.0), 0);
        assert_eq!(total_frames_from_duration(10, Rational::new(1, 30), 0.0), 0);
    }
}
