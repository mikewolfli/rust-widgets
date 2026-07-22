//! Video module — container format detection, metadata reading, frame extraction, and playback control.

pub mod decoder;
pub mod format;
pub mod frame;
pub mod metadata;
pub mod player;

#[cfg(feature = "video-codecs")]
pub mod ffmpeg_decoder;
pub mod engine;

pub use engine::VideoEngine;
pub use decoder::{FrameBufferDecoder, MjpegDecoder, VideoDecoder};
pub use format::ContainerFormat;
pub use frame::VideoFrame;
pub use metadata::VideoMetadata;
pub use player::PlaybackState;

#[cfg(feature = "video-codecs")]
pub use ffmpeg_decoder::{decode_frames, FfmpegDecoder};


