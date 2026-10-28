// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Quality adjustment configuration.
use super::level::QualityLevel;
/// Configuration for quality adjustment behavior.
///
/// # Examples
///
/// ```rust
/// use rust_widgets::quality::QualityConfig;
///
/// let config = QualityConfig::default();
/// assert!(config.target_frame_rate > 0.0);
/// assert!(config.degrade_threshold >= 1.0);
/// ```
#[derive(Debug, Clone, Copy)]
pub struct QualityConfig {
    /// Target frame rate in frames per second.
    pub target_frame_rate: f32,
    /// Threshold multiplier for degrading quality (e.g., 1.5 = degrade when frame time exceeds 1.5x target).
    pub degrade_threshold: f32,
    /// Threshold multiplier for upgrading quality (e.g., 0.7 = upgrade when frame time is below 0.7x target).
    pub upgrade_threshold: f32,
    /// Maximum allowed quality level.
    pub max_quality: QualityLevel,
    /// Minimum allowed quality level.
    pub min_quality: QualityLevel,
    /// Number of consecutive frames that must exceed threshold before degrading.
    pub degrade_frame_count: usize,
    /// Number of consecutive frames that must be below threshold before upgrading.
    pub upgrade_frame_count: usize,
}
impl Default for QualityConfig {
    fn default() -> Self {
        Self {
            target_frame_rate: 60.0,
            degrade_threshold: 1.5,
            upgrade_threshold: 0.7,
            max_quality: QualityLevel::High,
            min_quality: QualityLevel::Low,
            degrade_frame_count: 5,
            upgrade_frame_count: 10,
        }
    }
}
impl QualityConfig {
    /// Returns the target frame duration in seconds.
    pub fn target_frame_duration(&self) -> f32 {
        1.0 / self.target_frame_rate
    }
    /// Returns the frame duration threshold for degrading quality.
    pub fn degrade_frame_duration(&self) -> f32 {
        self.target_frame_duration() * self.degrade_threshold
    }
    /// Returns the frame duration threshold for upgrading quality.
    pub fn upgrade_frame_duration(&self) -> f32 {
        self.target_frame_duration() * self.upgrade_threshold
    }
    /// The history window a monitor needs to honour this config's consecutive-frame counts.
    ///
    /// The monitor can only look back over the samples its ring buffer holds, so a config that asks
    /// for more consecutive frames than the buffer holds would silently never fire. Deriving the
    /// buffer size from the counts keeps the two from disagreeing. Always at least one sample.
    pub fn required_history_capacity(&self) -> usize {
        self.degrade_frame_count.max(self.upgrade_frame_count).max(1)
    }
    /// Creates a new config with clamped threshold values.
    ///
    /// # The quality range is repaired too
    ///
    /// `min_quality`/`max_quality` are public, so a caller can invert them (`min_quality = High`
    /// with `max_quality = Low`) — and every level write clamps against the pair:
    /// `QualityLevel::clamp` **panics** on `min > max`. This method used to leave the pair alone
    /// despite being the function whose name promises a usable config, so the inversion survived
    /// normalization and reached the panic.
    ///
    /// `QualityLevel` orders `High > Medium > Low`, so "min" is the *greater* value and the
    /// repair swaps rather than comparing against `min`.
    pub fn normalized(self) -> Self {
        let (min_quality, max_quality) = if self.min_quality > self.max_quality {
            (self.max_quality, self.min_quality)
        } else {
            (self.min_quality, self.max_quality)
        };
        Self {
            // A public field can be constructed directly, so the target rate needs the same
            // guard as the thresholds: `0.0` gives an infinite frame duration (never degrade,
            // always upgrade), a negative rate gives a negative one (always degrade), and `NaN`
            // poisons every comparison. The default rate is the only honest repair (LYT-X07).
            target_frame_rate: if self.target_frame_rate.is_finite() && self.target_frame_rate > 0.0
            {
                self.target_frame_rate
            } else {
                60.0
            },
            degrade_threshold: self.degrade_threshold.max(1.0),
            upgrade_threshold: self.upgrade_threshold.clamp(0.1, 1.0),
            degrade_frame_count: self.degrade_frame_count.max(1),
            upgrade_frame_count: self.upgrade_frame_count.max(1),
            min_quality,
            max_quality,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quality::QualityManager;

    /// LYT-X07: `target_frame_rate` is public, so a caller can construct `0`, a negative value or
    /// `NaN` directly. Before the repair, `0` made the frame duration infinite (never degrade,
    /// always upgrade), a negative rate made it negative (always degrade), and `NaN` poisoned every
    /// comparison — the adaptive policy silently stopped adapting.
    #[test]
    fn normalized_repairs_an_unusable_target_frame_rate() {
        for rate in [0.0, -30.0, f32::NAN, f32::INFINITY] {
            let config = QualityConfig { target_frame_rate: rate, ..QualityConfig::default() };
            let fixed = config.normalized();
            assert_eq!(
                fixed.target_frame_rate, 60.0,
                "{rate} is not a frame rate, so it must fall back to the default"
            );
            assert!(
                fixed.target_frame_duration().is_finite() && fixed.target_frame_duration() > 0.0,
                "the repaired duration must be a real duration"
            );
        }
        // A usable rate is passed through unchanged.
        let kept = QualityConfig { target_frame_rate: 144.0, ..QualityConfig::default() };
        assert_eq!(kept.normalized().target_frame_rate, 144.0);
    }

    /// Regression: `min_quality`/`max_quality` are public, so a caller could invert them. Every
    /// level write clamps against the pair and `QualityLevel::clamp` **panics** on `min > max`;
    /// `normalized()` used to leave the inversion in place despite promising a usable config.
    #[test]
    fn normalized_repairs_inverted_quality_range() {
        let inverted = QualityConfig {
            min_quality: QualityLevel::High,
            max_quality: QualityLevel::Low,
            ..QualityConfig::default()
        };
        let fixed = inverted.normalized();
        assert!(fixed.min_quality <= fixed.max_quality);
        assert_eq!(fixed.min_quality, QualityLevel::Low);
        assert_eq!(fixed.max_quality, QualityLevel::High);
    }

    /// The whole point of the repair is that the panic is unreachable through the public
    /// constructor, so assert on the constructor rather than on `normalized()` alone.
    #[test]
    fn inverted_config_through_manager_does_not_panic() {
        let inverted = QualityConfig {
            min_quality: QualityLevel::High,
            max_quality: QualityLevel::Low,
            ..QualityConfig::default()
        };
        let mut manager = QualityManager::with_config(inverted);
        let _ = manager.quality_level();
        manager.set_quality_level(QualityLevel::Medium);
        let selected = manager.quality_level();
        assert!(selected >= manager.config().min_quality);
        assert!(selected <= manager.config().max_quality);

        // `set_config` is the other entry point that must repair rather than propagate.
        manager.set_config(inverted);
        manager.set_quality_level(QualityLevel::High);
        let selected = manager.quality_level();
        assert!(selected >= manager.config().min_quality);
        assert!(selected <= manager.config().max_quality);
    }

    /// Non-quality fields were already clamped; keep them pinned so the repair above is not
    /// mistaken for licence to loosen them.
    #[test]
    fn normalized_clamps_thresholds_and_counts() {
        let config = QualityConfig {
            degrade_threshold: 0.1,
            upgrade_threshold: 5.0,
            degrade_frame_count: 0,
            upgrade_frame_count: 0,
            ..QualityConfig::default()
        };
        let fixed = config.normalized();
        assert_eq!(fixed.degrade_threshold, 1.0);
        assert_eq!(fixed.upgrade_threshold, 1.0);
        assert_eq!(fixed.degrade_frame_count, 1);
        assert_eq!(fixed.upgrade_frame_count, 1);
    }
}
