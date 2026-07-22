//! Real audio output using cpal (cross-platform audio library).
//! Gated behind `#[cfg(feature = "audio-output")]`.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

use crate::audio::samples::AudioBuffer;

/// Audio output device that plays audio through the system speakers.
pub struct AudioOutput {
    device: Option<cpal::Device>,
    config: Option<cpal::StreamConfig>,
    stream: Option<cpal::Stream>,
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
        Ok(Self { device: Some(device), config: Some(config.into()), stream: None })
    }

    /// Play an AudioBuffer through the default output device.
    /// Returns the stream handle; playback continues until the buffer finishes or the stream is dropped.
    pub fn play(&mut self, buffer: &AudioBuffer) -> Result<(), String> {
        let device = self.device.as_ref().ok_or("No audio device")?;
        let config = self.config.as_ref().ok_or("No audio config")?;

        let samples = buffer.samples.clone();

        let err_fn = |err| log::error!("Audio stream error: {}", err);
        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    for (dst, src) in data.iter_mut().zip(samples.iter()) {
                        *dst = *src;
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

    /// Stop playback.
    pub fn stop(&mut self) {
        self.stream = None;
    }

    /// Returns the name of the default output device, if available.
    pub fn device_name(&self) -> Option<String> {
        self.device.as_ref().and_then(|d| d.name().ok())
    }
}
