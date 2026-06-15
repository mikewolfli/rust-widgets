//! Real video decoder powered by FFmpeg (via ffmpeg-next).
//! Gated behind `#[cfg(feature = "video-codecs")]` which provides
//! hardware-accelerated decoding for MP4, AVI, MKV, WebM, FLV, WMV, MOV
//! and many other container formats.
//!
//! Decodes frames to RGBA8 format and populates full `VideoMetadata`.

use std::collections::VecDeque;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use ffmpeg_next::codec::decoder::Video as VideoDecoder;
use ffmpeg_next::format::context::Input;
use ffmpeg_next::media;
use ffmpeg_next::software;
use ffmpeg_next::util::format;
use ffmpeg_next::Rational;

use crate::video::format::ContainerFormat;
use crate::video::frame::{FrameType, VideoFrame};
use crate::video::metadata::VideoMetadata;
use crate::video::VideoDecoder as VideoDecoderTrait;

// ---------------------------------------------------------------------------
// Atomic counter for unique temp-file names
// ---------------------------------------------------------------------------

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn next_temp_path() -> PathBuf {
    let count = TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let pid = std::process::id();
    let mut path = std::env::temp_dir();
    path.push(format!("rust_widgets_ffmpeg_{pid}_{count}.tmp"));
    path
}

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
    let mut frames = Vec::with_capacity(metadata.total_frames as usize);

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
    /// Temporary file path (cleaned up on drop).
    _temp_path: Option<PathBuf>,

    // ── streaming state ──────────────────────────────────────────────
    /// True when the demuxer has been fully consumed.
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
    pub fn new(data: Vec<u8>) -> Result<Self, String> {
        ffmpeg_next::init().map_err(|e| format!("FFmpeg init failed: {e}"))?;

        // Write data to a temporary file so ffmpeg-next can open it.
        let temp_path = next_temp_path();
        let mut file =
            fs::File::create(&temp_path).map_err(|e| format!("Failed to create temp file: {e}"))?;
        file.write_all(&data).map_err(|e| format!("Failed to write temp file: {e}"))?;
        file.flush().map_err(|e| format!("Failed to flush temp file: {e}"))?;

        let result = Self::from_path(&temp_path);

        // If construction failed, clean up the temp file immediately.
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }

        result
    }

    /// Open an FFmpeg decoder from a file path.
    fn from_path(path: &std::path::Path) -> Result<Self, String> {
        let input =
            ffmpeg_next::format::input(path).map_err(|e| format!("Failed to open input: {e}"))?;

        // Find the best video stream.
        let stream = input
            .streams()
            .best(media::Type::Video)
            .ok_or_else(|| "No video stream found".to_string())?;

        let stream_index = stream.index();
        let time_base = stream.time_base();

        // Build codec parameters → decoder context.
        let codec_ctx = ffmpeg_next::codec::Context::from_parameters(stream.parameters())
            .map_err(|e| format!("Failed to create decoder context: {e}"))?;

        let decoder = codec_ctx
            .decoder()
            .video()
            .map_err(|e| format!("Failed to open video decoder: {e}"))?;

        // Create the RGBA scaler.
        let scaler = software::converter(
            (decoder.width(), decoder.height()),
            decoder.format(),
            format::Pixel::RGBA,
        )
        .map_err(|e| format!("Failed to create scaler: {e}"))?;

        let metadata = build_metadata(&input, &decoder, &stream);

        Ok(Self {
            input,
            decoder,
            scaler,
            metadata,
            stream_index,
            time_base,
            _temp_path: Some(path.to_path_buf()),
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

        self.scaler.run(frame, &mut rgb).map_err(|e| format!("Scaler failed: {e}"))?;

        // The scaler has now allocated the output frame; read its data.
        let data = rgb.data(0).to_vec();

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
                Err(e) => return Err(format!("Failed to read packet: {e}")),
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
                        Err(e) => log::warn!("[FfmpegDecoder] frame conversion skipped: {e}"),
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
        // Convert seconds to stream time base.
        let ts =
            (time * self.time_base.denominator() as f64 / self.time_base.numerator() as f64) as i64;

        self.input.seek(ts, ..).map_err(|e| format!("Seek failed: {e}"))?;

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
        if let Some(path) = self._temp_path.take() {
            let _ = fs::remove_file(&path);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
        stream_frames as u64
    } else if fps > 0.0 {
        let dur = stream.duration();
        if dur > 0 {
            (dur as f64 * rational.numerator() as f64
                / (rational.denominator() as f64 * stream.time_base().denominator() as f64))
                .round() as u64
        } else {
            0
        }
    } else {
        0
    };

    (fps, total)
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
        ffmpeg_next::codec::Id::V308 => "v308".into(),
        ffmpeg_next::codec::Id::V408 => "v408".into(),
        ffmpeg_next::codec::Id::V410 => "v410".into(),
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
        let result = FfmpegDecoder::new(vec![0u8; 100]);
        assert!(result.is_err(), "expected error for invalid video data");
    }

    #[test]
    fn test_decode_invalid_mp4() {
        // An ftyp box with no moov → should fail gracefully.
        let data = small_mp4_data();
        let result = FfmpegDecoder::new(data);
        assert!(result.is_err(), "expected error for header-only MP4");
    }

    #[test]
    fn test_decode_empty() {
        let result = FfmpegDecoder::new(vec![]);
        assert!(result.is_err());
    }

    #[test]
    fn test_standalone_decode_frames_empty() {
        let result = decode_frames(b"");
        assert!(result.is_err());
    }

    #[test]
    fn test_standalone_decode_frames_invalid() {
        let result = decode_frames(&[0u8; 256]);
        assert!(result.is_err());
    }

    #[test]
    fn test_codec_name_known() {
        assert_eq!(codec_name_from_id(ffmpeg_next::codec::Id::H264), "h264");
        assert_eq!(codec_name_from_id(ffmpeg_next::codec::Id::VP9), "vp9");
        assert_eq!(codec_name_from_id(ffmpeg_next::codec::Id::MJPEG), "mjpeg");
    }
}
