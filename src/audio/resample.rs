//! Audio resampling — sample rate conversion.

use crate::audio::samples::AudioBuffer;

/// Resample audio buffer to a new sample rate using linear interpolation.
pub fn resample(buffer: &AudioBuffer, target_sample_rate: u32) -> AudioBuffer {
    if buffer.sample_rate == target_sample_rate || buffer.sample_rate == 0 {
        return buffer.clone();
    }

    let ratio = target_sample_rate as f64 / buffer.sample_rate as f64;
    let new_frames = (buffer.frames() as f64 * ratio) as usize;
    let mut new_samples = Vec::with_capacity(new_frames * buffer.channels as usize);

    for ch in 0..buffer.channels as usize {
        let ch_data = buffer.channel(ch);
        for i in 0..new_frames {
            let src_pos = i as f64 / ratio;
            let src_idx = src_pos as usize;
            let frac = src_pos - src_idx as f64;

            if src_idx + 1 < ch_data.len() {
                let val =
                    ch_data[src_idx] * (1.0 - frac as f32) + ch_data[src_idx + 1] * frac as f32;
                new_samples.push(val);
            } else if src_idx < ch_data.len() {
                new_samples.push(ch_data[src_idx]);
            }
        }
    }

    // Re-interleave
    let ch = buffer.channels as usize;
    let frames = new_samples.len() / ch;
    let mut interleaved = Vec::with_capacity(new_samples.len());
    for f in 0..frames {
        for c in 0..ch {
            let idx = c * frames + f;
            interleaved.push(new_samples.get(idx).copied().unwrap_or(0.0));
        }
    }

    AudioBuffer::new(target_sample_rate, interleaved, buffer.channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resample_same_rate() {
        let buf = AudioBuffer::new(44100, vec![0.0; 1000], 1);
        let result = resample(&buf, 44100);
        assert_eq!(result.sample_rate, 44100);
        assert_eq!(result.samples.len(), 1000);
    }

    #[test]
    fn test_resample_downsample() {
        let buf = AudioBuffer::new(44100, vec![0.0; 44100], 1);
        let result = resample(&buf, 22050);
        assert_eq!(result.sample_rate, 22050);
        assert!(result.samples.len() < 44100);
        assert!(result.samples.len() > 20000);
    }

    #[test]
    fn test_resample_upsample() {
        let buf = AudioBuffer::new(22050, vec![0.0; 22050], 1);
        let result = resample(&buf, 44100);
        assert_eq!(result.sample_rate, 44100);
        assert!(result.samples.len() > 40000);
    }

    #[test]
    fn test_resample_stereo() {
        let buf = AudioBuffer::new(44100, vec![0.0; 88200], 2);
        let result = resample(&buf, 22050);
        assert_eq!(result.channels, 2);
        assert!(result.samples.len() < 88200);
    }

    #[test]
    fn test_resample_zero_rate() {
        let buf = AudioBuffer::new(0, vec![], 1);
        let result = resample(&buf, 44100);
        assert_eq!(result.sample_rate, 0);
    }
}
