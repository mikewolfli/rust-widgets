use crate::core::Color;
use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    BounceIn,
    BounceOut,
    ElasticIn,
    ElasticOut,
    BackIn,
    BackOut,
}

impl Default for EasingFunction {
    fn default() -> Self {
        Self::Linear
    }
}

impl EasingFunction {
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EaseIn => t * t,
            Self::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Self::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Self::BounceIn => 1.0 - Self::BounceOut.apply(1.0 - t),
            Self::BounceOut => {
                if t < 1.0 / 2.75 {
                    7.5625 * t * t
                } else if t < 2.0 / 2.75 {
                    let t = t - 1.5 / 2.75;
                    7.5625 * t * t + 0.75
                } else if t < 2.5 / 2.75 {
                    let t = t - 2.25 / 2.75;
                    7.5625 * t * t + 0.9375
                } else {
                    let t = t - 2.625 / 2.75;
                    7.5625 * t * t + 0.984375
                }
            }
            Self::ElasticIn => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    -(2.0_f32.powf(10.0 * (t - 1.0)))
                        * ((t - 1.1) * 5.0 * std::f32::consts::PI).sin()
                }
            }
            Self::ElasticOut => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    2.0_f32.powf(-10.0 * t) * ((t - 0.1) * 5.0 * std::f32::consts::PI).sin() + 1.0
                }
            }
            Self::BackIn => {
                const C1: f32 = 1.70158;
                const C3: f32 = C1 + 1.0;
                C3 * t * t * t - C1 * t * t
            }
            Self::BackOut => {
                const C1: f32 = 1.70158;
                const C3: f32 = C1 + 1.0;
                1.0 + C3 * (t - 1.0).powi(3) + C1 * (t - 1.0).powi(2)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationDirection {
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}

impl Default for AnimationDirection {
    fn default() -> Self {
        Self::Normal
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationFillMode {
    None,
    Forwards,
    Backwards,
    Both,
}

impl Default for AnimationFillMode {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Debug, Clone)]
pub struct AnimationConfig {
    pub duration: Duration,
    pub delay: Duration,
    pub easing: EasingFunction,
    pub direction: AnimationDirection,
    pub fill_mode: AnimationFillMode,
    pub iteration_count: u32,
    pub infinite: bool,
}

impl AnimationConfig {
    pub fn new(duration: Duration) -> Self {
        Self {
            duration,
            delay: Duration::ZERO,
            easing: EasingFunction::Linear,
            direction: AnimationDirection::Normal,
            fill_mode: AnimationFillMode::None,
            iteration_count: 1,
            infinite: false,
        }
    }

    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }

    pub fn with_direction(mut self, direction: AnimationDirection) -> Self {
        self.direction = direction;
        self
    }

    pub fn with_fill_mode(mut self, fill_mode: AnimationFillMode) -> Self {
        self.fill_mode = fill_mode;
        self
    }

    pub fn with_iterations(mut self, count: u32) -> Self {
        self.iteration_count = count;
        self.infinite = false;
        self
    }

    pub fn infinite(mut self) -> Self {
        self.infinite = true;
        self
    }
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self::new(Duration::from_millis(300))
    }
}

pub struct Animation {
    config: AnimationConfig,
    start_time: Option<Instant>,
    is_running: bool,
    is_paused: bool,
    current_iteration: u32,
}

impl Animation {
    pub fn new(config: AnimationConfig) -> Self {
        Self {
            config,
            start_time: None,
            is_running: false,
            is_paused: false,
            current_iteration: 0,
        }
    }

    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        self.is_running = true;
        self.is_paused = false;
        self.current_iteration = 0;
    }

    pub fn stop(&mut self) {
        self.is_running = false;
        self.start_time = None;
        self.current_iteration = 0;
    }

    pub fn pause(&mut self) {
        self.is_paused = true;
    }

    pub fn resume(&mut self) {
        self.is_paused = false;
    }

    pub fn is_running(&self) -> bool {
        self.is_running && !self.is_paused
    }

    pub fn is_completed(&self) -> bool {
        if self.config.infinite {
            false
        } else {
            self.current_iteration >= self.config.iteration_count
        }
    }

    pub fn progress(&self) -> f32 {
        if !self.is_running {
            return 0.0;
        }

        let elapsed = self.start_time.map(|t| t.elapsed()).unwrap_or_default();

        if elapsed < self.config.delay {
            return 0.0;
        }

        let animation_elapsed = elapsed - self.config.delay;
        let raw_progress = animation_elapsed.as_secs_f32() / self.config.duration.as_secs_f32();

        let progress = if self.config.infinite {
            raw_progress % 1.0
        } else {
            (raw_progress % 1.0).min(1.0)
        };

        let eased_progress = self.config.easing.apply(progress);

        match self.config.direction {
            AnimationDirection::Normal => eased_progress,
            AnimationDirection::Reverse => 1.0 - eased_progress,
            AnimationDirection::Alternate => {
                if self.current_iteration % 2 == 0 {
                    eased_progress
                } else {
                    1.0 - eased_progress
                }
            }
            AnimationDirection::AlternateReverse => {
                if self.current_iteration % 2 == 0 {
                    1.0 - eased_progress
                } else {
                    eased_progress
                }
            }
        }
    }

    pub fn update(&mut self) {
        if !self.is_running || self.is_paused {
            return;
        }

        let elapsed = self.start_time.map(|t| t.elapsed()).unwrap_or_default();

        if elapsed > self.config.delay {
            let animation_elapsed = elapsed - self.config.delay;
            let raw_progress = animation_elapsed.as_secs_f32() / self.config.duration.as_secs_f32();

            if raw_progress >= 1.0 {
                self.current_iteration = raw_progress as u32;

                if !self.config.infinite && self.current_iteration >= self.config.iteration_count {
                    self.is_running = false;
                }
            }
        }
    }

    pub fn config(&self) -> &AnimationConfig {
        &self.config
    }
}

pub struct ColorAnimation {
    animation: Animation,
    from_color: Color,
    to_color: Color,
}

impl ColorAnimation {
    pub fn new(config: AnimationConfig, from: Color, to: Color) -> Self {
        Self {
            animation: Animation::new(config),
            from_color: from,
            to_color: to,
        }
    }

    pub fn start(&mut self) {
        self.animation.start();
    }

    pub fn stop(&mut self) {
        self.animation.stop();
    }

    pub fn current_color(&self) -> Color {
        let progress = self.animation.progress();
        Self::interpolate_color(self.from_color, self.to_color, progress)
    }

    pub fn update(&mut self) {
        self.animation.update();
    }

    pub fn is_running(&self) -> bool {
        self.animation.is_running()
    }

    fn interpolate_color(from: Color, to: Color, t: f32) -> Color {
        let r = ((1.0 - t) * from.r as f32 + t * to.r as f32) as u8;
        let g = ((1.0 - t) * from.g as f32 + t * to.g as f32) as u8;
        let b = ((1.0 - t) * from.b as f32 + t * to.b as f32) as u8;
        let a = ((1.0 - t) * from.a as f32 + t * to.a as f32) as u8;
        Color::rgba(r, g, b, a)
    }
}

pub struct FloatAnimation {
    animation: Animation,
    from_value: f32,
    to_value: f32,
}

impl FloatAnimation {
    pub fn new(config: AnimationConfig, from: f32, to: f32) -> Self {
        Self {
            animation: Animation::new(config),
            from_value: from,
            to_value: to,
        }
    }

    pub fn start(&mut self) {
        self.animation.start();
    }

    pub fn stop(&mut self) {
        self.animation.stop();
    }

    pub fn current_value(&self) -> f32 {
        let progress = self.animation.progress();
        self.from_value + (self.to_value - self.from_value) * progress
    }

    pub fn update(&mut self) {
        self.animation.update();
    }

    pub fn is_running(&self) -> bool {
        self.animation.is_running()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_easing_functions() {
        assert_eq!(EasingFunction::Linear.apply(0.5), 0.5);
        assert!(EasingFunction::EaseIn.apply(0.5) < 0.5);
        assert!(EasingFunction::EaseOut.apply(0.5) > 0.5);
    }

    #[test]
    fn test_animation_config() {
        let config = AnimationConfig::new(Duration::from_millis(500))
            .with_delay(Duration::from_millis(100))
            .with_easing(EasingFunction::EaseInOut)
            .with_iterations(3);

        assert_eq!(config.duration, Duration::from_millis(500));
        assert_eq!(config.delay, Duration::from_millis(100));
        assert_eq!(config.iteration_count, 3);
    }

    #[test]
    fn test_color_animation() {
        let config = AnimationConfig::new(Duration::from_millis(100));
        let mut animation = ColorAnimation::new(config, Color::RED, Color::BLUE);

        animation.start();
        let color = animation.current_color();

        assert!(color.r < 255 || color.b > 0);
    }

    #[test]
    fn test_float_animation() {
        let config = AnimationConfig::new(Duration::from_millis(100));
        let mut animation = FloatAnimation::new(config, 0.0, 100.0);

        animation.start();
        let value = animation.current_value();

        assert!(value >= 0.0 && value <= 100.0);
    }
}
