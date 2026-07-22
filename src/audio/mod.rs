//! Audio module — format detection, decoding, encoding, sample processing, and normalization.

pub mod decoder;
pub mod encoder;
pub mod engine;
pub mod format;
pub mod normalize;
pub mod resample;
pub mod samples;

#[cfg(feature = "video-codecs")]
pub mod ffmpeg_encoder;

#[cfg(feature = "audio-output")]
pub mod output;

pub use decoder::{decode, detect_audio_format};
pub use encoder::encode;
pub use engine::AudioEngine;
pub use format::AudioFormat;
pub use normalize::normalize;
pub use resample::resample;
pub use samples::AudioBuffer;

#[cfg(feature = "video-codecs")]
pub use ffmpeg_encoder::ffmpeg_encode;

#[cfg(feature = "audio-output")]
pub use output::AudioOutput;
