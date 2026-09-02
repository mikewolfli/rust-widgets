//! Real audio output using cpal (cross-platform audio library).
//! Gated behind `#[cfg(feature = "audio-output")]`.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::audio::samples::AudioBuffer;

/// Audio output device that plays audio through the system speakers.
pub struct AudioOutput {
    device: Option<cpal::Device>,
    config: Option<cpal::StreamConfig>,
    stream: Option<cpal::Stream>,
    /// Set once the audio callback has exhausted the buffer's samples.
    finished: Arc<AtomicBool>,
}

impl AudioOutput {
    /// Create a new audio output connected to the default output device.
    pub fn new() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "No default audio output device found".to_string())?;
        let config = device
            .default_output_config()
            .map_err(|e| format!("Failed to get default output config: {}", e))?;
        Ok(Self {
            device: Some(device),
            config: Some(config.into()),
            stream: None,
            finished: Arc::new(AtomicBool::new(false)),
        })
    }

    /// Play an AudioBuffer through the default output device.
    ///
    /// Returns the stream handle; playback continues until the buffer
    /// finishes or the stream is dropped. When the buffer runs out of
    /// samples the callback logs `audio buffer exhausted` once and emits
    /// silence: cpal 0.15's `Stream` is neither `Send` nor `Sync`, so the
    /// audio callback (which must be `Send + 'static`) cannot hold a handle
    /// to pause the stream itself. Callers should observe [`Self::is_finished`]
    /// and call [`Self::stop`] (or drop the stream) when playback is done.
    pub fn play(&mut self, buffer: &AudioBuffer) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("No audio device")?;
        let config = self.config.as_ref().ok_or("No audio config")?;

        let samples = buffer.samples.clone();
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
                    for slot in data.iter_mut() {
                        if written < samples.len() {
                            *slot = samples[written];
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
            .map_err(|e| format!("Failed to build audio stream: {}", e))?;

        stream.play().map_err(|e| format!("Failed to start audio stream: {}", e))?;
        self.stream = Some(stream);
        Ok(())
    }

    /// Returns true once the audio buffer has been fully consumed.
    pub fn is_finished(&self) -> bool {
        self.finished.load(Ordering::Relaxed)
    }

    /// Stop playback.
    pub fn stop(&mut self) {
        self.stream = None;
        self.finished.store(false, Ordering::Relaxed);
    }

    /// Returns the name of the default output device, if available.
    pub fn device_name(&self) -> Option<String> {
        self.device.as_ref().and_then(|d| d.name().ok())
    }
}
