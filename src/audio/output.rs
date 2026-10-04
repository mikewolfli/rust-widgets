// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Real audio output using cpal (cross-platform audio library).
//! Gated behind `#[cfg(feature = "audio-output")]`.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::audio::samples::AudioBuffer;

/// Audio output device that plays audio through the system speakers.
pub struct AudioOutput {
    device: Option<cpal::Device>,
    config: Option<cpal::StreamConfig>,
    /// The sample format reported by the device's default output config.
    sample_format: cpal::SampleFormat,
    stream: Option<cpal::Stream>,
    /// Set once the audio callback has exhausted the buffer's samples.
    finished: Arc<AtomicBool>,
    /// Playback volume as the raw bits of an `f32`. Shared with the audio
    /// callback so volume changes take effect without rebuilding the stream.
    volume: Arc<AtomicU32>,
    /// Whether the stream is currently paused.
    paused: bool,
}

impl AudioOutput {
    /// Create a new audio output connected to the default output device.
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host.default_output_device().ok_or_else(|| {
            "no default audio output device: the host reported no audio device, so playback \
             cannot be started — check that an output device is enabled in the OS sound settings"
                .to_string()
        })?;
        let supported = device.default_output_config().map_err(|e| {
            format!(
                "default output device '{}' has no usable output config: {e}",
                device.name().unwrap_or_else(|_| "<unnamed>".to_string())
            )
        })?;
        // Capture the sample format before consuming the supported config; the
        // plain StreamConfig carries sample rate/channels but not the format.
        let sample_format = supported.sample_format();
        let config: cpal::StreamConfig = supported.into();
        Ok(Self {
            device: Some(device),
            config: Some(config),
            sample_format,
            stream: None,
            finished: Arc::new(AtomicBool::new(false)),
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            paused: false,
        })
    }

    /// Play an AudioBuffer through the default output device.
    ///
    /// The buffer is converted to the device's sample rate and channel count
    /// before playback, so it is consumed at the correct speed and layout
    /// rather than being played at the wrong rate through the wrong channel
    /// count. Only F32 output is supported (the buffer is stored as F32); a
    /// device that only reports a different sample format is rejected with an
    /// explicit error instead of being fed the wrong bit depth.
    ///
    /// Playback continues until the buffer finishes or the stream is dropped.
    /// When the buffer runs out of samples the callback sets [`Self::is_finished`]
    /// and emits silence: cpal 0.15's `Stream` is neither `Send` nor `Sync`, so
    /// the audio callback cannot hold a handle back to pause the stream itself.
    /// Callers should observe [`Self::is_finished`] and call [`Self::stop`] (or
    /// drop the stream) when playback is done.
    pub fn play(&mut self, buffer: &AudioBuffer) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("No audio device")?;
        let config = self.config.as_ref().ok_or("No audio config")?;

        if self.sample_format != cpal::SampleFormat::F32 {
            return Err(format!(
                "output device '{}' reports sample format {:?}, but only F32 is supported: \
                 resampling and channel conversion happen in the F32 domain",
                self.device_name().unwrap_or_else(|| "<unnamed>".to_string()),
                self.sample_format
            ));
        }

        // Convert to the device's sample rate/channels so the samples are read
        // at the correct speed and layout. Without this a 24 kHz mono buffer
        // played through a 48 kHz stereo device would run at half speed with
        // only one of the two channels populated.
        let prepared = prepare_buffer(buffer, config.sample_rate.0, config.channels as u8);
        let samples = prepared.samples.clone();
        let volume = Arc::clone(&self.volume);
        let finished = Arc::clone(&self.finished);
        finished.store(false, Ordering::Relaxed);

        // Callback state: `written`/`warned_exhausted` persist across callbacks
        // because the closure is `FnMut`.
        let mut written = 0usize;
        let mut warned_exhausted = false;
        let err_fn = |err| log::error!("Audio stream error: {}", err);
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    // Copy samples in order across callbacks; once the buffer
                    // is exhausted, emit silence and report it exactly once so
                    // playback never becomes silent forever without a trace.
                    let gain = f32::from_bits(volume.load(Ordering::Relaxed));
                    for slot in data.iter_mut() {
                        if written < samples.len() {
                            *slot = samples[written] * gain;
                            written += 1;
                        } else {
                            *slot = 0.0;
                            if !warned_exhausted {
                                warned_exhausted = true;
                                finished.store(true, Ordering::Relaxed);
                                log::warn!(
                                    "audio buffer exhausted: no samples left to play; \
                                     emitting silence — stop or drop the stream to end playback"
                                );
                            }
                        }
                    }
                },
                err_fn,
                None,
            )
            .map_err(|e| {
                format!(
                    "output stream could not be built for config {config:?}: {e} (check that \
                     the device is still connected and supports this config)"
                )
            })?;

        stream.play().map_err(|e| {
            format!("output stream could not be started for config {config:?}: {e}")
        })?;
        self.stream = Some(stream);
        self.paused = false;
        Ok(())
    }

    /// Pause playback. The stream is kept so it can be resumed later.
    pub fn pause(&mut self) {
        if let Some(stream) = &self.stream {
            if let Err(e) = stream.pause() {
                log::error!("Failed to pause audio stream: {e}");
            }
        }
        self.paused = true;
    }

    /// Resume a paused stream.
    pub fn resume(&mut self) {
        if let Some(stream) = &self.stream {
            if let Err(e) = stream.play() {
                log::error!("Failed to resume audio stream: {e}");
            }
        }
        self.paused = false;
    }

    /// Returns true if the stream is currently paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Set playback volume (0.0 to 1.0).
    pub fn set_volume(&mut self, volume: f32) {
        self.volume.store(volume.clamp(0.0, 1.0).to_bits(), Ordering::Relaxed);
    }

    /// Get the current playback volume.
    pub fn volume(&self) -> f32 {
        f32::from_bits(self.volume.load(Ordering::Relaxed))
    }

    /// Returns true once the audio buffer has been fully consumed.
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    /// Stop playback.
    pub fn stop(&mut self) {
        self.stream = None;
        self.finished.store(false, Ordering::Relaxed);
        self.paused = false;
    }

    /// Returns the name of the default output device, if available.
    pub fn device_name(&self) -> Option<String> {
        self.device.as_ref().and_then(|d| d.name().ok())
    }

    /// Build an output with no device attached. Used by tests to exercise the
    /// state machine (pause/stop/volume/finished) and the explicit failure path
    /// without requiring a real sound card.
    #[cfg(test)]
    pub(crate) fn new_without_device() -> Self {
        Self {
            device: None,
            config: None,
            sample_format: cpal::SampleFormat::F32,
            stream: None,
            finished: Arc::new(AtomicBool::new(false)),
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            paused: false,
        }
    }

    /// Force the finished flag for tests, simulating the audio callback
    /// reporting that the buffer has been exhausted.
    #[cfg(test)]
    pub(crate) fn mark_finished_for_test(&self) {
        self.finished.store(true, Ordering::Relaxed);
    }
}

/// Convert a buffer to the target sample rate and channel count.
///
/// Sample-rate conversion is delegated to [`crate::audio::resample::resample`];
/// channel conversion is a deterministic F32-domain remap. This is the pure
/// "conversion decision" that turns a mismatched buffer into the exact layout
/// the device expects, and it is unit-tested independently of any sound card.
fn prepare_buffer(buffer: &AudioBuffer, target_rate: u32, target_channels: u8) -> AudioBuffer {
    let resampled = crate::audio::resample::resample(buffer, target_rate);
    let converted = convert_channels(
        &resampled.samples,
        resampled.channels() as usize,
        target_channels as usize,
    );
    AudioBuffer::new(target_rate, converted, target_channels)
}

/// Convert interleaved samples between channel counts.
///
/// - Mono -> N duplicates the single channel.
/// - N -> mono averages all channels.
/// - Otherwise each target channel reuses the nearest source channel (dropping
///   extras when downmixing, duplicating the last when upmixing).
fn convert_channels(samples: &[f32], src_channels: usize, dst_channels: usize) -> Vec<f32> {
    if src_channels == dst_channels || src_channels == 0 {
        return samples.to_vec();
    }
    let frames = samples.len() / src_channels;
    let mut out = Vec::with_capacity(frames * dst_channels);
    for frame in 0..frames {
        let base = frame * src_channels;
        if dst_channels == 1 {
            let sum: f32 = samples[base..base + src_channels].iter().sum();
            out.push(sum / src_channels as f32);
        } else if src_channels == 1 {
            let s = samples[base];
            for _ in 0..dst_channels {
                out.push(s);
            }
        } else {
            for dst in 0..dst_channels {
                let src = dst.min(src_channels - 1);
                out.push(samples[base + src]);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn convert_channels_passthrough() {
        let out = convert_channels(&[1.0, 2.0, 3.0, 4.0], 2, 2);
        assert_eq!(out, vec![1.0, 2.0, 3.0, 4.0]);
    }

    #[test]
    fn convert_channels_mono_to_stereo_duplicates() {
        let out = convert_channels(&[1.0, 2.0], 1, 2);
        assert_eq!(out, vec![1.0, 1.0, 2.0, 2.0]);
    }

    #[test]
    fn convert_channels_stereo_to_mono_averages() {
        let out = convert_channels(&[1.0, 3.0, 0.0, 2.0], 2, 1);
        assert_eq!(out, vec![2.0, 1.0]);
    }

    #[test]
    fn prepare_buffer_resamples_and_converts_channels() {
        // 1 second of 24 kHz mono played through a 48 kHz stereo device must
        // become 48 kHz stereo and keep a ~1 second duration.
        let buf = AudioBuffer::new(24000, vec![0.0f32; 24000], 1);
        let prepared = prepare_buffer(&buf, 48000, 2);
        assert_eq!(prepared.sample_rate, 48000);
        assert_eq!(prepared.channels(), 2);
        // 24000 frames * 2 (rate ratio) * 2 (channels) = 96000 samples.
        assert_eq!(prepared.samples.len(), 96000);
    }

    #[test]
    fn prepare_buffer_same_rate_and_channels_is_passthrough() {
        let buf = AudioBuffer::new(44100, vec![0.5, -0.5, 1.0, -1.0], 2);
        let prepared = prepare_buffer(&buf, 44100, 2);
        assert_eq!(prepared.samples, buf.samples);
        assert_eq!(prepared.channels(), 2);
    }

    #[test]
    fn play_without_device_fails_explicitly() {
        let mut out = AudioOutput::new_without_device();
        let buf = AudioBuffer::new(44100, vec![0.0; 4], 1);
        let err = out.play(&buf).unwrap_err();
        assert!(err.contains("No audio device"), "got: {err}");
    }

    #[test]
    fn volume_defaults_to_unity_and_set_volume_round_trips() {
        let mut out = AudioOutput::new_without_device();
        assert_eq!(out.volume(), 1.0);
        out.set_volume(0.25);
        assert!((out.volume() - 0.25).abs() < 1e-6);
        out.set_volume(2.0);
        assert_eq!(out.volume(), 1.0);
        out.set_volume(-1.0);
        assert_eq!(out.volume(), 0.0);
    }

    #[test]
    fn pause_resume_and_stop_track_state() {
        let mut out = AudioOutput::new_without_device();
        assert!(!out.is_paused());
        out.pause();
        assert!(out.is_paused());
        out.resume();
        assert!(!out.is_paused());
        out.pause();
        out.stop();
        assert!(!out.is_paused());
        assert!(!out.is_finished());
    }

    #[test]
    fn finished_flag_is_observable() {
        let out = AudioOutput::new_without_device();
        assert!(!out.is_finished());
        out.mark_finished_for_test();
        assert!(out.is_finished());
    }
}
