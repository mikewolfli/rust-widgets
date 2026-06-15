//! Audio normalization — RMS and Peak normalization.

use crate::audio::samples::AudioBuffer;

/// Normalize audio buffer to a target level.
pub enum NormalizationTarget {
    /// Peak normalization: scale so the maximum absolute value reaches this level (0.0-1.0).
    Peak(f32),
    /// RMS normalization: scale so the RMS value reaches this level (0.0-1.0).
    Rms(f32),
}

/// Apply normalization to an audio buffer. Modifies in-place.
pub fn normalize(buffer: &mut AudioBuffer, target: NormalizationTarget) {
    if buffer.samples.is_empty() {
        return;
    }

    let gain = match target {
        NormalizationTarget::Peak(target_peak) => {
            let current_peak = buffer.samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max);
            if current_peak > 0.0 {
                (target_peak / current_peak).min(10.0)
            } else {
                1.0
            }
        }
        NormalizationTarget::Rms(target_rms) => {
            let rms = compute_rms(&buffer.samples);
            if rms > 0.0 {
                (target_rms / rms).min(10.0)
            } else {
                1.0
            }
        }
    };

    if (gain - 1.0).abs() > f32::EPSILON {
        for sample in &mut buffer.samples {
            *sample *= gain;
        }
    }
}

/// Compute the RMS (Root Mean Square) value of samples.
pub fn compute_rms(samples: &[f32]) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let sum_sq: f32 = samples.iter().map(|s| s * s).sum();
    (sum_sq / samples.len() as f32).sqrt()
}

/// Compute the peak (maximum absolute) value of samples.
pub fn compute_peak(samples: &[f32]) -> f32 {
    samples.iter().map(|s| s.abs()).fold(0.0f32, f32::max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_peak_normalization() {
        let mut buf = AudioBuffer::new(44100, vec![0.5, -0.3, 0.8, -0.2], 1);
        normalize(&mut buf, NormalizationTarget::Peak(1.0));
        let peak = compute_peak(&buf.samples);
        assert!((peak - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_rms_normalization() {
        let mut buf = AudioBuffer::new(44100, vec![0.5, -0.3, 0.8, -0.2], 1);
        normalize(&mut buf, NormalizationTarget::Rms(0.5));
        let rms = compute_rms(&buf.samples);
        assert!((rms - 0.5).abs() < 0.01);
    }

    #[test]
    fn test_normalize_empty() {
        let mut buf = AudioBuffer::new(44100, vec![], 1);
        normalize(&mut buf, NormalizationTarget::Peak(1.0)); // Should not crash
        assert!(buf.samples.is_empty());
    }

    #[test]
    fn test_normalize_silence() {
        let mut buf = AudioBuffer::new(44100, vec![0.0; 100], 1);
        normalize(&mut buf, NormalizationTarget::Peak(1.0)); // Should not crash or NaN
        assert!(buf.samples.iter().all(|s| s.is_finite()));
    }

    #[test]
    fn test_compute_rms() {
        let samples = vec![1.0, -1.0, 1.0, -1.0];
        let rms = compute_rms(&samples);
        assert!((rms - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_compute_peak() {
        let samples = vec![0.1, -0.5, 0.3, -0.9, 0.2];
        let peak = compute_peak(&samples);
        assert!((peak - 0.9).abs() < f32::EPSILON);
    }
}
