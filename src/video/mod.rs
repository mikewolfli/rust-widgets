// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Video module — container format detection, metadata reading, frame extraction, and playback control.
//!
//! # Reachability
//!
//! **State:** Production callers: `src/widget/special_widgets/media_player.rs:42` (the master clock),
//! `src/video/engine.rs:176` (`VideoEngine::tick_frame`). The decoder itself is behind `--features video`.

/// Codec-agnostic decoder interface plus the built-in MJPEG and
/// frame-buffer decoders.
pub mod decoder;
/// Container format sniffing (MP4, WebM, AVI, MJPEG, …).
pub mod format;
/// The decoded-frame type handed to the renderer.
pub mod frame;
/// Title, dimensions, duration, and frame-rate information read from a
/// container without decoding the video stream.
pub mod metadata;
pub mod player;

/// Container-level streaming engine that drives a decoder and manages
/// playback state.
pub mod engine;
#[cfg(feature = "video-codecs")]
pub mod ffmpeg_decoder;

pub use decoder::{FrameBufferDecoder, MjpegDecoder, VideoDecoder};
pub use engine::VideoEngine;
pub use format::ContainerFormat;
pub use frame::VideoFrame;
pub use metadata::VideoMetadata;
pub use player::PlaybackState;

#[cfg(feature = "video-codecs")]
pub use ffmpeg_decoder::{decode_frames, FfmpegDecoder};
