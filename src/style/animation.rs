// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::HashMap;
use crate::compat::{Duration, Instant};
use crate::core::Color;
use crate::style::theme_state::{StatefulTheme, WidgetState};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// The timing curve applied to an animation's raw progress.
///
/// [`EasingFunction::apply`] maps a normalized input `t` in `0.0..=1.0` to an
/// eased output that is *usually* in the same range, but not always: [`Self::BackIn`],
/// [`Self::BackOut`], [`Self::ElasticIn`] and [`Self::ElasticOut`] deliberately
/// overshoot outside it, which is how the "pull back" and "spring past" effects
/// are produced. Input outside `0.0..=1.0` is clamped first.
pub enum EasingFunction {
    #[default]
    /// Constant speed: the output equals the input.
    Linear,
    /// Accelerates from rest; the output lags the input in the first half.
    EaseIn,
    /// Decelerates to rest; the output leads the input in the first half.
    EaseOut,
    /// Accelerates out of the start and decelerates into the end.
    EaseInOut,
    /// [`Self::BounceOut`] played backwards, so the motion collides at the end.
    BounceIn,
    /// Decelerates into the target with a series of diminishing bounces.
    BounceOut,
    /// [`Self::ElasticOut`] played backwards, so the motion snaps into the
    /// start with increasing oscillation.
    ElasticIn,
    /// Overshoots the target and oscillates back to it, like a stretched spring.
    ElasticOut,
    /// Starts by moving *away* from the target before accelerating toward it.
    /// Overshoots below `0.0`.
    BackIn,
    /// Overshoots past the target before settling back. Overshoots above `1.0`.
    BackOut,
}
impl EasingFunction {
    /// Maps normalized progress `t` through this curve.
    ///
    /// `t` is clamped to `0.0..=1.0` before evaluation, so every variant yields
    /// exactly `0.0` at `t <= 0.0` and exactly `1.0` at `t >= 1.0`. The result of
    /// an intermediate `t` may fall outside `0.0..=1.0` for the overshooting
    /// variants listed on [`EasingFunction`].
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// Which direction each iteration of a repeating animation runs in.
///
/// Only [`Self::Alternate`] and [`Self::AlternateReverse`] depend on the
/// iteration index; the other two play every iteration identically, so a
/// non-infinite animation with them simply restarts from the same end.
pub enum AnimationDirection {
    #[default]
    /// Play forwards on every iteration: progress runs `0.0` to `1.0`.
    Normal,
    /// Play backwards on every iteration: progress runs `1.0` to `0.0`.
    Reverse,
    /// Alternate: forwards on even iterations, backwards on odd ones.
    Alternate,
    /// Alternate starting from backwards: backwards on even iterations,
    /// forwards on odd ones.
    AlternateReverse,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// What an animation's value is before it starts and after it finishes.
///
/// This is a declaration carried on [`AnimationConfig`]; the animators in this
/// module read the `to`/`from` values directly rather than consulting the mode,
/// so treat it as a hint for whatever layer consumes the animation.
pub enum AnimationFillMode {
    #[default]
    /// Apply no value outside the active interval: before the delay the
    /// animation has no effect, and it is dropped once finished.
    None,
    /// Retain the final value after the animation finishes.
    Forwards,
    /// Apply the first value during the delay, before the animation starts.
    Backwards,
    /// Both [`Self::Backwards`] and [`Self::Forwards`].
    Both,
}
#[derive(Debug, Clone)]
/// The timing and repetition parameters for one animation.
///
pub struct AnimationConfig {
    /// How long one iteration lasts. A zero duration is handled by the animators,
    /// which treat it as an instant jump rather than dividing by it.
    pub duration: Duration,
    /// How long to wait before the first iteration begins. Progress reads `0.0`
    /// for the whole delay.
    pub delay: Duration,
    /// The timing curve applied to each iteration's progress.
    pub easing: EasingFunction,
    /// Which direction iteration 0 runs in, and whether later iterations flip.
    pub direction: AnimationDirection,
    /// What value the animation leaves behind before and after its active interval.
    pub fill_mode: AnimationFillMode,
    /// How many iterations to run before the animation counts as complete.
    ///
    /// Ignored while `infinite` is set. [`AnimationConfig::new`] starts this at
    /// `1`, so an animation plays once by default.
    pub iteration_count: u32,
    /// Run forever: [`Animation::is_completed`] always returns `false` and
    /// `iteration_count` is not consulted.
    pub infinite: bool,
}
impl AnimationConfig {
    /// Creates a config running one forward, linear iteration over `duration`,
    /// with no delay, no fill and no repetition.
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
    /// Returns a copy with the start delay set to `delay`.
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }
    /// Returns a copy using `easing` as the timing curve.
    pub fn with_easing(mut self, easing: EasingFunction) -> Self {
        self.easing = easing;
        self
    }
    /// Returns a copy running in `direction`.
    pub fn with_direction(mut self, direction: AnimationDirection) -> Self {
        self.direction = direction;
        self
    }
    /// Returns a copy with `fill_mode` applied.
    pub fn with_fill_mode(mut self, fill_mode: AnimationFillMode) -> Self {
        self.fill_mode = fill_mode;
        self
    }
    /// Returns a copy running `count` iterations, and clears the infinite flag.
    ///
    /// The reset matters because `infinite` otherwise wins: without it,
    /// `config.infinite().with_iterations(3)` would still never complete.
    pub fn with_iterations(mut self, count: u32) -> Self {
        self.iteration_count = count;
        self.infinite = false;
        self
    }
    /// Returns a copy that repeats forever, ignoring `iteration_count`.
    pub fn infinite(mut self) -> Self {
        self.infinite = true;
        self
    }
}
impl Default for AnimationConfig {
    /// A one-shot 300 ms linear animation with no delay — the conventional UI
    /// transition length.
    fn default() -> Self {
        Self::new(Duration::from_millis(300))
    }
}
/// A single animation instance: config plus the clock state needed to drive it.
///
/// The clock is advanced by explicit calls to [`Animation::update`] rather than a
/// timer, so how smooth the result looks depends on how often the host calls it.
/// Reading [`Animation::progress`] does not advance anything and is safe from
/// `&self`.
pub struct Animation {
    config: AnimationConfig,
    start_time: Option<Instant>,
    is_running: bool,
    is_paused: bool,
    /// Tracks when the animation was paused, used on resume to adjust start_time.
    pause_start_time: Option<Instant>,
    current_iteration: u32,
    on_complete_callback: Option<Box<dyn FnMut()>>,
    /// Progress frozen at the moment of pause, so progress() returns the
    /// correct value while the animation is paused.
    frozen_progress: Option<f32>,
}
impl Animation {
    /// Creates a stopped animation in iteration `0` with no completion callback.
    pub fn new(config: AnimationConfig) -> Self {
        Self {
            config,
            start_time: None,
            is_running: false,
            is_paused: false,
            pause_start_time: None,
            current_iteration: 0,
            on_complete_callback: None,
            frozen_progress: None,
        }
    }
    /// Starts the animation from now, clearing any pause and resetting the
    /// iteration counter to `0`.
    pub fn start(&mut self) {
        self.start_time = Some(Instant::now());
        self.is_running = true;
        self.is_paused = false;
        self.current_iteration = 0;
    }
    /// Stops the animation and resets its clock, leaving it at iteration `0`.
    ///
    /// The completion callback fires here, even though the animation did not
    /// reach its end, and is consumed: a later [`Animation::start`] will not
    /// invoke it again until [`Animation::on_complete`] installs a new one.
    pub fn stop(&mut self) {
        self.is_running = false;
        self.start_time = None;
        self.current_iteration = 0;
        self.frozen_progress = None;
        if let Some(mut callback) = self.on_complete_callback.take() {
            callback();
        }
    }
    /// Freezes progress at its current value and records the pause instant.
    ///
    /// The animation is not advanced by [`Animation::update`] while paused, so
    /// progress holds at the frozen value until [`Animation::resume`].
    pub fn pause(&mut self) {
        self.frozen_progress = Some(self.compute_progress());
        self.is_paused = true;
        self.pause_start_time = Some(Instant::now());
    }
    /// Reports whether the animation is currently paused.
    ///
    /// A paused animation is still "started": [`Animation::is_running`] returns
    /// `false` while this does.
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }
    /// Returns the animation to the state [`Animation::new`] left it in:
    /// stopped, unpaused, iteration `0`, no frozen progress.
    ///
    /// The completion callback is *not* cleared and does not fire.
    pub fn reset(&mut self) {
        self.is_running = false;
        self.is_paused = false;
        self.start_time = None;
        self.pause_start_time = None;
        self.current_iteration = 0;
        self.frozen_progress = None;
    }
    /// Sets the callback invoked once when the animation completes or is stopped.
    ///
    /// Setting a new callback replaces the old one. The callback is single-shot:
    /// it is taken and consumed the first time it fires.
    pub fn on_complete(&mut self, callback: Box<dyn FnMut()>) {
        self.on_complete_callback = Some(callback);
    }
    /// Un-pauses the animation and shifts its start instant forward by however
    /// long the pause lasted, so progress picks up where it stopped rather than
    /// jumping ahead.
    ///
    /// Calling this on an animation that is not paused is harmless; if the
    /// animation was never started there is no start instant to shift.
    pub fn resume(&mut self) {
        self.is_paused = false;
        self.frozen_progress = None;
        // Adjust start_time forward by the paused duration so the animation
        // resumes from where it stopped rather than jumping forward.
        if let Some(pause_start) = self.pause_start_time.take() {
            let paused_duration = pause_start.elapsed();
            if let Some(ref mut start) = self.start_time {
                *start += paused_duration;
            }
        }
    }
    /// Reports whether the animation is advancing: started and not paused.
    pub fn is_running(&self) -> bool {
        self.is_running && !self.is_paused
    }
    /// Reports whether every requested iteration has been played.
    ///
    /// Always `false` for an infinite animation. This is derived from the
    /// iteration counter, which only [`Animation::update`] advances, so an
    /// animation that was started but never updated still reads as incomplete.
    pub fn is_completed(&self) -> bool {
        if self.config.infinite {
            false
        } else {
            self.current_iteration >= self.config.iteration_count
        }
    }
    /// The animation's position in its current iteration, as an eased
    /// `0.0..=1.0` fraction (outside that range for an overshooting easing curve).
    ///
    /// Returns `0.0` while the animation is stopped or still in its delay, and
    /// returns the frozen value while paused. Within an iteration the raw
    /// progress wraps, so a non-infinite animation that has overrun its last
    /// iteration reads `1.0` and an infinite one restarts the range.
    pub fn progress(&self) -> f32 {
        if self.is_paused {
            // Return the progress frozen at the moment pause() was called.
            return self.frozen_progress.unwrap_or(0.0);
        }
        if !self.is_running {
            return 0.0;
        }
        let elapsed = self.start_time.map(|t| t.elapsed()).unwrap_or_default();
        if elapsed < self.config.delay {
            return 0.0;
        }
        let animation_elapsed = elapsed - self.config.delay;
        // Guard against division by zero when duration is ZERO (default).
        let duration_secs = self.config.duration.as_secs_f32().max(f32::EPSILON);
        let raw_progress = animation_elapsed.as_secs_f32() / duration_secs;
        let progress =
            if self.config.infinite { raw_progress % 1.0 } else { (raw_progress % 1.0).min(1.0) };
        let eased_progress = self.config.easing.apply(progress);
        match self.config.direction {
            AnimationDirection::Normal => eased_progress,
            AnimationDirection::Reverse => 1.0 - eased_progress,
            AnimationDirection::Alternate => {
                if self.current_iteration.is_multiple_of(2) {
                    eased_progress
                } else {
                    1.0 - eased_progress
                }
            }
            AnimationDirection::AlternateReverse => {
                if self.current_iteration.is_multiple_of(2) {
                    1.0 - eased_progress
                } else {
                    eased_progress
                }
            }
        }
    }
    /// Advances the clock: recomputes the iteration counter and finishes the
    /// animation once its last iteration has elapsed.
    ///
    /// Does nothing while the animation is stopped or paused. Call it once per
    /// frame; the completion callback fires from here rather than from a timer.
    /// Unlike [`Animation::progress`], this path has no zero-duration guard.
    pub fn update(&mut self) {
        if !self.is_running || self.is_paused {
            // Paused animations keep their frozen progress and do
            // not advance their iteration count.
            return;
        }
        let elapsed = self.start_time.map(|t| t.elapsed()).unwrap_or_default();
        if elapsed > self.config.delay {
            let animation_elapsed = elapsed - self.config.delay;
            let raw_progress = animation_elapsed.as_secs_f32() / self.config.duration.as_secs_f32();
            if raw_progress >= 1.0 {
                self.current_iteration = raw_progress.floor() as u32;
                if !self.config.infinite && self.current_iteration >= self.config.iteration_count {
                    self.is_running = false;
                    if let Some(mut callback) = self.on_complete_callback.take() {
                        callback();
                    }
                }
            }
        }
    }
    /// Borrows the configuration this animation runs with.
    pub fn config(&self) -> &AnimationConfig {
        &self.config
    }

    /// Compute the current progress without considering pause state.
    /// This is used to capture the progress value at the moment of pause.
    fn compute_progress(&self) -> f32 {
        if !self.is_running {
            return 0.0;
        }
        let elapsed = self.start_time.map(|t| t.elapsed()).unwrap_or_default();
        if elapsed < self.config.delay {
            return 0.0;
        }
        let animation_elapsed = elapsed - self.config.delay;
        let duration_secs = self.config.duration.as_secs_f32().max(f32::EPSILON);
        let raw_progress = animation_elapsed.as_secs_f32() / duration_secs;
        let progress =
            if self.config.infinite { raw_progress % 1.0 } else { (raw_progress % 1.0).min(1.0) };
        let eased_progress = self.config.easing.apply(progress);
        match self.config.direction {
            AnimationDirection::Normal => eased_progress,
            AnimationDirection::Reverse => 1.0 - eased_progress,
            AnimationDirection::Alternate => {
                if self.current_iteration.is_multiple_of(2) {
                    eased_progress
                } else {
                    1.0 - eased_progress
                }
            }
            AnimationDirection::AlternateReverse => {
                if self.current_iteration.is_multiple_of(2) {
                    1.0 - eased_progress
                } else {
                    eased_progress
                }
            }
        }
    }
}
/// Animates between two colours, interpolating each RGBA channel independently.
pub struct ColorAnimation {
    animation: Animation,
    from_color: Color,
    to_color: Color,
}
impl ColorAnimation {
    /// Creates a stopped colour animation running from `from` to `to`.
    pub fn new(config: AnimationConfig, from: Color, to: Color) -> Self {
        Self { animation: Animation::new(config), from_color: from, to_color: to }
    }
    /// Starts the animation from now.
    pub fn start(&mut self) {
        self.animation.start();
    }
    /// Stops the animation, firing its completion callback if one was set.
    pub fn stop(&mut self) {
        self.animation.stop();
    }
    /// The colour at the animation's current progress.
    ///
    /// The blend is linear in the channels' 8-bit values, so it traverses the
    /// RGB cube rather than a perceptual colour space. Reads `from` when stopped,
    /// since progress is then `0.0`.
    pub fn current_color(&self) -> Color {
        let progress = self.animation.progress();
        Self::interpolate_color(self.from_color, self.to_color, progress)
    }
    /// Advances the animation. Call once per frame.
    pub fn update(&mut self) {
        self.animation.update();
    }
    /// Reports whether the animation is running and not paused.
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
/// Animates a single `f32` between two values.
pub struct FloatAnimation {
    animation: Animation,
    from_value: f32,
    to_value: f32,
}
impl FloatAnimation {
    /// Creates a stopped float animation running from `from` to `to`.
    pub fn new(config: AnimationConfig, from: f32, to: f32) -> Self {
        Self { animation: Animation::new(config), from_value: from, to_value: to }
    }
    /// Starts the animation from now.
    pub fn start(&mut self) {
        self.animation.start();
    }
    /// Stops the animation, firing its completion callback if one was set.
    pub fn stop(&mut self) {
        self.animation.stop();
    }
    /// The interpolated value at the animation's current progress.
    ///
    /// Reads `from_value` when stopped, since progress is then `0.0`.
    pub fn current_value(&self) -> f32 {
        let progress = self.animation.progress();
        self.from_value + (self.to_value - self.from_value) * progress
    }
    /// Advances the animation. Call once per frame.
    pub fn update(&mut self) {
        self.animation.update();
    }
    /// Reports whether the animation is running and not paused.
    pub fn is_running(&self) -> bool {
        self.animation.is_running()
    }
}
/// Describes a single animated property on a target object.
#[derive(Debug, Clone)]
pub struct PropertyAnimation {
    /// Unique animation ID.
    pub id: AnimationId,
    /// Property name (e.g., "x", "y", "width", "opacity", "rotation").
    pub property: String,
    /// Starting value.
    pub from: f32,
    /// Ending value.
    pub to: f32,
    /// Current value (updated each frame).
    pub current: f32,
}

/// A unique identifier for an active animation managed by `AnimationDriver`.
pub type AnimationId = u64;

/// Callback invoked each frame with the current animation progress (0.0–1.0).
pub type AnimationTickCallback = Box<dyn FnMut(f32)>;

/// Callback invoked when an animation completes.
pub type AnimationCompleteCallback = Box<dyn FnMut()>;

/// A tracked active animation within the `AnimationDriver`.
struct ActiveAnimation {
    /// The core animation state machine.
    anim: Animation,
    /// Callback invoked every tick with the un-eased progress [0,1].
    tick: Option<AnimationTickCallback>,
    /// Callback invoked once when the animation finishes.
    on_complete: Option<AnimationCompleteCallback>,
    /// The last reported progress to avoid redundant callbacks.
    last_progress: f32,
}

/// Global animation driver that manages and advances active animations.
///
/// Call `advance()` from your event loop or render loop to tick all animations.
/// Use `add()` to register a new animation with a progress callback.
pub struct AnimationDriver {
    animations: HashMap<AnimationId, ActiveAnimation>,
    property_animations: HashMap<AnimationId, PropertyAnimation>,
    next_id: AnimationId,
}

impl AnimationDriver {
    /// Creates a new empty animation driver.
    pub fn new() -> Self {
        Self { animations: HashMap::new(), property_animations: HashMap::new(), next_id: 1 }
    }

    /// Register a new animation and return its ID.
    /// The `tick` callback is called every frame with the eased progress \[0,1\].
    pub fn add<F>(&mut self, config: AnimationConfig, tick: F) -> AnimationId
    where
        F: FnMut(f32) + 'static,
    {
        let id = self.next_id;
        self.next_id += 1;
        let mut anim = Animation::new(config);
        anim.start();
        self.animations.insert(
            id,
            ActiveAnimation {
                anim,
                tick: Some(Box::new(tick)),
                on_complete: None,
                last_progress: -1.0,
            },
        );
        id
    }

    /// Register a float animation that interpolates between `from` and `to`.
    /// The `on_tick` callback receives the current interpolated value each frame.
    pub fn add_float<F>(
        &mut self,
        config: AnimationConfig,
        from: f32,
        to: f32,
        mut on_tick: F,
    ) -> AnimationId
    where
        F: FnMut(f32) + 'static,
    {
        let range = to - from;
        self.add(config, move |progress| {
            on_tick(from + range * progress);
        })
    }

    /// Register a color animation that interpolates between `from` and `to`.
    /// The `on_tick` callback receives the current interpolated color each frame.
    pub fn add_color<F>(
        &mut self,
        config: AnimationConfig,
        from: Color,
        to: Color,
        mut on_tick: F,
    ) -> AnimationId
    where
        F: FnMut(Color) + 'static,
    {
        self.add(config, move |progress| {
            let r = ((1.0 - progress) * from.r as f32 + progress * to.r as f32) as u8;
            let g = ((1.0 - progress) * from.g as f32 + progress * to.g as f32) as u8;
            let b = ((1.0 - progress) * from.b as f32 + progress * to.b as f32) as u8;
            let a = ((1.0 - progress) * from.a as f32 + progress * to.a as f32) as u8;
            on_tick(Color::rgba(r, g, b, a));
        })
    }

    /// Advance all animations by one frame.
    /// Calls tick callbacks with updated progress and removes completed animations.
    /// Returns the number of still-active (running) animations.
    pub fn advance(&mut self) -> usize {
        // Phase 1: Advance internal animation state (iteration counting,
        // on_complete callbacks) by calling update() on every active animation.
        // This must happen before progress snapshots so is_completed() is accurate.
        for entry in self.animations.values_mut() {
            if entry.anim.is_running() {
                entry.anim.update();
            }
        }

        // Snapshot: collect progress without mutable borrows
        let snap: Vec<(AnimationId, f32)> = self
            .animations
            .iter()
            .filter(|(_, e)| e.anim.is_running())
            .map(|(&id, e)| (id, e.anim.progress()))
            .collect();
        // Fire tick callbacks with mutable access
        for (id, progress) in &snap {
            if let Some(entry) = self.animations.get_mut(id) {
                if (progress - entry.last_progress).abs() > 0.001 || *progress == 0.0_f32 {
                    if let Some(ref mut cb) = entry.tick {
                        cb(*progress);
                    }
                    entry.last_progress = *progress;
                }
            }
        }
        // Update PropertyAnimation.current values so callers reading
        // .current on a PropertyAnimation get the latest interpolated value.
        for (id, progress) in &snap {
            if let Some(prop_anim) = self.property_animations.get_mut(id) {
                let range = prop_anim.to - prop_anim.from;
                prop_anim.current = prop_anim.from + range * progress;
            }
        }
        // Remove completed animations
        let finished: Vec<AnimationId> = self
            .animations
            .iter()
            .filter(|(_, e)| e.anim.is_completed() && !e.anim.config().infinite)
            .map(|(&id, _)| id)
            .collect();
        for id in &finished {
            if let Some(mut entry) = self.animations.remove(id) {
                if let Some(mut cb) = entry.on_complete.take() {
                    cb();
                }
            }
        }
        self.animations.len()
    }

    /// Remove and stop a specific animation.
    pub fn remove(&mut self, id: AnimationId) {
        self.animations.remove(&id);
    }

    /// Remove all active animations.
    pub fn clear(&mut self) {
        self.animations.clear();
    }

    /// Returns the number of active (running) animations.
    pub fn len(&self) -> usize {
        self.animations.len()
    }

    /// Returns true if there are no active animations.
    pub fn is_empty(&self) -> bool {
        self.animations.is_empty()
    }

    /// Start animating a named property from `from` to `to` over `duration`.
    /// The `on_tick` callback receives the current value and progress (0.0–1.0).
    pub fn animate<F>(
        &mut self,
        property: impl Into<String>,
        from: f32,
        to: f32,
        duration: Duration,
        easing: EasingFunction,
        mut on_tick: F,
    ) -> AnimationId
    where
        F: FnMut(f32, f32) + 'static,
    {
        let prop: String = property.into();
        let config = AnimationConfig::new(duration).with_easing(easing);
        let range = to - from;
        let id = self.add(config, move |progress| {
            let value = from + range * progress;
            on_tick(value, progress);
        });
        // Store property metadata so callers can inspect active animations
        let current = from;
        self.property_animations
            .insert(id, PropertyAnimation { id, property: prop.clone(), from, to, current });
        id
    }

    /// Convenience: animate with linear easing.
    pub fn animate_linear<F>(
        &mut self,
        property: impl Into<String>,
        from: f32,
        to: f32,
        duration: Duration,
        on_tick: F,
    ) -> AnimationId
    where
        F: FnMut(f32, f32) + 'static,
    {
        self.animate(property, from, to, duration, EasingFunction::Linear, on_tick)
    }

    /// Convenience: animate with ease-in-out easing.
    pub fn animate_ease<F>(
        &mut self,
        property: impl Into<String>,
        from: f32,
        to: f32,
        duration: Duration,
        on_tick: F,
    ) -> AnimationId
    where
        F: FnMut(f32, f32) + 'static,
    {
        self.animate(property, from, to, duration, EasingFunction::EaseInOut, on_tick)
    }

    /// Returns the current progress of an animation, or `None` if not found.
    pub fn get_progress(&self, id: AnimationId) -> Option<f32> {
        self.animations.get(&id).map(|entry| entry.anim.progress())
    }

    /// Returns the `PropertyAnimation` metadata for a given animation ID, if any.
    ///
    /// Only animations created via [`animate`](AnimationDriver::animate) (or its
    /// convenience wrappers) will have a `PropertyAnimation` entry.
    pub fn get_property_animation(&self, id: AnimationId) -> Option<&PropertyAnimation> {
        self.property_animations.get(&id)
    }
}

crate::impl_default_via_new!(AnimationDriver);

/// Animate a state transition using a StatefulTheme's transition duration.
/// Returns the AnimationId if a transition was found, or None if no transition is configured.
pub fn animate_state_transition<F>(
    driver: &mut AnimationDriver,
    theme: &StatefulTheme,
    from: WidgetState,
    to: WidgetState,
    on_tick: F,
) -> Option<AnimationId>
where
    F: FnMut(f32, f32) + 'static,
{
    let duration_ms = theme.get_transition(&from, &to)?;
    let duration = Duration::from_millis(duration_ms as u64);
    Some(driver.animate_linear(format!("state_{from:?}_to_{to:?}"), 0.0, 1.0, duration, on_tick))
}

/// A group of animations that run concurrently.
///
/// This is a grouping handle, not a scheduler: the child animations are still
/// owned and advanced by [`AnimationDriver`], and the group only remembers their
/// IDs so they can be waited on together. Removing a child from the driver
/// makes this group consider it complete, because a missing animation has no
/// progress to check.
pub struct ParallelAnimation {
    ids: Vec<AnimationId>,
}

impl ParallelAnimation {
    /// Creates a group with no children.
    pub fn new() -> Self {
        Self { ids: Vec::new() }
    }
}

crate::impl_default_via_new!(ParallelAnimation);

impl ParallelAnimation {
    /// Add a child animation (managed externally via AnimationDriver).
    pub fn add(&mut self, id: AnimationId) {
        self.ids.push(id);
    }

    /// Returns true if all child animations have completed.
    pub fn is_completed(&self, driver: &AnimationDriver) -> bool {
        self.ids.iter().all(|id| driver.get_progress(*id).map(|p| p >= 1.0).unwrap_or(true))
    }

    /// Returns the number of child animations.
    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Reports whether the group has no children. A group with no children is
    /// also treated as completed by [`ParallelAnimation::is_completed`].
    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// A group of animations that run sequentially (one after another).
///
/// Child configs are stored, not live animations: each one is started on the
/// driver only when the previous one reports full progress. That single-child
/// handoff is also the main limitation — a child whose [`AnimationConfig`] is
/// infinite would stall the sequence forever.
pub struct SequentialAnimation {
    animations: Vec<AnimationConfig>,
    current_index: usize,
    current_id: Option<AnimationId>,
}

impl SequentialAnimation {
    /// Creates a sequence with no steps, positioned before the first one.
    pub fn new() -> Self {
        Self { animations: Vec::new(), current_index: 0, current_id: None }
    }
}

crate::impl_default_via_new!(SequentialAnimation);

impl SequentialAnimation {
    /// Add an animation config to the sequence.
    pub fn add(&mut self, config: AnimationConfig) {
        self.animations.push(config);
    }

    /// Start or advance the sequence. Call this each frame.
    /// Returns true if the sequence is still running.
    pub fn advance<F>(&mut self, driver: &mut AnimationDriver, mut on_tick: F) -> bool
    where
        F: FnMut(usize, f32) + 'static,
    {
        if self.current_index >= self.animations.len() {
            return false; // All done
        }
        // Check if current animation is done or not started
        match self.current_id {
            None => {
                // Start next animation
                let config = self.animations[self.current_index].clone();
                let idx = self.current_index;
                let id = driver.add(config, move |p| {
                    on_tick(idx, p);
                });
                self.current_id = Some(id);
                true
            }
            Some(id) => {
                if driver.get_progress(id).map(|p| p >= 1.0).unwrap_or(true) {
                    // Current animation done, move to next
                    self.current_index += 1;
                    self.current_id = None;
                    if self.current_index < self.animations.len() {
                        let config = self.animations[self.current_index].clone();
                        let idx = self.current_index;
                        let id = driver.add(config, move |p| {
                            on_tick(idx, p);
                        });
                        self.current_id = Some(id);
                    }
                    self.current_index < self.animations.len()
                } else {
                    true // still running
                }
            }
        }
    }

    /// Resets the sequence to its first step, so the next
    /// [`SequentialAnimation::advance`] starts it over.
    ///
    /// Any animation the sequence already registered keeps running on the
    /// driver; the sequence simply forgets it and starts a fresh one.
    pub fn reset(&mut self) {
        self.current_index = 0;
        self.current_id = None;
    }

    /// Returns the number of animation configs in the sequence.
    pub fn len(&self) -> usize {
        self.animations.len()
    }

    /// Returns `true` if the sequence contains no animations.
    pub fn is_empty(&self) -> bool {
        self.animations.is_empty()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BLUE11 R6.1: Keyframe Animation
// ═══════════════════════════════════════════════════════════════════════════

/// A single keyframe at a specific progress point.
#[derive(Debug, Clone)]
pub struct Keyframe {
    /// Normalized time [0.0, 1.0] for this keyframe.
    pub time: f32,
    /// Value at this keyframe.
    pub value: f32,
    /// Easing function to use when interpolating FROM this keyframe to the next.
    pub easing: EasingFunction,
}

/// A multi-keyframe animation that interpolates through a series of waypoints.
pub struct KeyframeAnimation {
    keyframes: Vec<Keyframe>,
    animation: Animation,
    config: AnimationConfig,
}

impl KeyframeAnimation {
    /// Create a new keyframe animation with the given keyframes and config.
    /// Keyframes should be sorted by time ascending.
    pub fn new(keyframes: Vec<Keyframe>, config: AnimationConfig) -> Self {
        Self { keyframes, config: config.clone(), animation: Animation::new(config) }
    }

    /// Start the animation.
    pub fn start(&mut self) {
        self.animation.start();
    }
    /// Stop the animation and reset progress.
    pub fn stop(&mut self) {
        self.animation.stop();
    }
    /// Pause the animation.
    pub fn pause(&mut self) {
        self.animation.pause();
    }
    /// Resume from pause.
    pub fn resume(&mut self) {
        self.animation.resume();
    }
    /// Returns a reference to the animation config.
    pub fn config(&self) -> &AnimationConfig {
        &self.config
    }

    /// Returns true if the animation is currently running.
    pub fn is_running(&self) -> bool {
        self.animation.is_running()
    }

    /// Evaluate the current interpolated value based on animation progress.
    pub fn current_value(&self) -> f32 {
        let progress = self.animation.progress();
        self.evaluate(progress)
    }

    /// Evaluate the animation at a given normalized progress [0.0, 1.0].
    pub fn evaluate(&self, progress: f32) -> f32 {
        if self.keyframes.is_empty() {
            return 0.0;
        }
        let progress = progress.clamp(0.0, 1.0);
        if progress <= self.keyframes[0].time {
            return self.keyframes[0].value;
        }
        if progress >= self.keyframes.last().unwrap().time {
            return self.keyframes.last().unwrap().value;
        }
        for i in 0..self.keyframes.len() - 1 {
            let kf_a = &self.keyframes[i];
            let kf_b = &self.keyframes[i + 1];
            if progress >= kf_a.time && progress <= kf_b.time {
                let segment_duration = kf_b.time - kf_a.time;
                if segment_duration == 0.0 {
                    return kf_b.value;
                }
                let local_t = (progress - kf_a.time) / segment_duration;
                let eased_t = kf_a.easing.apply(local_t);
                return kf_a.value + (kf_b.value - kf_a.value) * eased_t;
            }
        }
        self.keyframes.last().unwrap().value
    }

    /// Update animation state (call each frame).
    pub fn update(&mut self) {
        self.animation.update();
    }
    /// Returns true when the animation has completed.
    pub fn is_completed(&self) -> bool {
        self.animation.is_completed()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BLUE11 R6.2: CSS-style Transition Animation
// ═══════════════════════════════════════════════════════════════════════════

/// A CSS-style transition rule mapping a property to an animation config.
#[derive(Debug, Clone)]
pub struct TransitionRule {
    /// Property name pattern.
    pub property: String,
    /// Animation configuration (duration, easing, delay).
    pub config: AnimationConfig,
}

impl TransitionRule {
    /// Creates a rule for `property` that transitions over `duration` with
    /// `easing`, with no delay.
    pub fn new(property: impl Into<String>, duration: Duration, easing: EasingFunction) -> Self {
        Self {
            property: property.into(),
            config: AnimationConfig::new(duration).with_easing(easing),
        }
    }
    /// Returns a copy that waits `delay` before starting each transition.
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.config = self.config.clone().with_delay(delay);
        self
    }
}

/// Manages CSS-style transitions for a set of properties.
/// When a property value changes, smoothly interpolates to the new value.
#[derive(Debug, Clone)]
pub struct TransitionManager {
    transitions: HashMap<String, ActiveTransition>,
    rules: HashMap<String, TransitionRule>,
}

#[derive(Debug, Clone)]
struct ActiveTransition {
    from: f32,
    to: f32,
    current: f32,
    elapsed: Duration,
    rule: TransitionRule,
    completed: bool,
}

impl TransitionManager {
    /// Create a new empty transition manager.
    pub fn new() -> Self {
        Self { transitions: HashMap::new(), rules: HashMap::new() }
    }
    /// Add a transition rule for a property.
    pub fn add_rule(&mut self, rule: TransitionRule) {
        self.rules.insert(rule.property.clone(), rule);
    }
    /// Remove a transition rule for a property.
    pub fn remove_rule(&mut self, property: &str) {
        self.rules.remove(property);
    }
    /// Check if a transition rule exists.
    pub fn has_rule(&self, property: &str) -> bool {
        self.rules.contains_key(property)
    }

    /// Begin a transition for a property from its old value to a new value.
    pub fn transition_to(&mut self, property: impl Into<String>, value: f32) {
        let prop = property.into();
        if let Some(rule) = self.rules.get(&prop) {
            let current = self.transitions.get(&prop).map(|t| t.current).unwrap_or(value);
            self.transitions.insert(
                prop,
                ActiveTransition {
                    from: current,
                    to: value,
                    current,
                    elapsed: Duration::ZERO,
                    rule: rule.clone(),
                    completed: false,
                },
            );
        }
    }

    /// Advance all active transitions by `dt`. Returns true if any still active.
    pub fn advance(&mut self, dt: Duration) -> bool {
        let mut any_active = false;
        for t in self.transitions.values_mut() {
            if t.completed {
                continue;
            }
            t.elapsed += dt;
            if t.elapsed >= t.rule.config.delay {
                let active_time = t.elapsed - t.rule.config.delay;
                let duration_s = t.rule.config.duration.as_secs_f32();
                let progress = if duration_s > 0.0 {
                    (active_time.as_secs_f32() / duration_s).clamp(0.0, 1.0)
                } else {
                    1.0
                };
                let eased = t.rule.config.easing.apply(progress);
                t.current = t.from + (t.to - t.from) * eased;
                if progress >= 1.0 {
                    t.current = t.to;
                    t.completed = true;
                } else {
                    any_active = true;
                }
            } else {
                any_active = true;
            }
        }
        self.transitions.retain(|_, t| !t.completed);
        any_active
    }
    /// Get the current value of a transitioning property.
    pub fn current_value(&self, property: &str) -> Option<f32> {
        self.transitions.get(property).map(|t| t.current)
    }
    /// The number of child animations currently mid-transition.
    pub fn len(&self) -> usize {
        self.transitions.len()
    }
    /// Reports whether no property is currently transitioning.
    pub fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }
    /// Drops every in-flight transition and their values, keeping the registered
    /// rules so later changes to the same properties still animate.
    pub fn clear(&mut self) {
        self.transitions.clear();
    }
}

crate::impl_default_via_new!(TransitionManager);

/// Physical spring animation (iOS-style spring physics).
pub struct SpringAnimation {
    animation: Animation,
    from_value: f32,
    to_value: f32,
    stiffness: f32,
    damping: f32,
    mass: f32,
    velocity: f32,
    current_value: f32,
}

impl SpringAnimation {
    /// Creates a stopped spring running from `from` to `to`.
    ///
    /// The defaults are stiffness `200.0`, damping `20.0` and mass `1.0`. The
    /// inner [`Animation`] is given a 1-second duration, but the spring's own
    /// integration decides when it settles, so that duration does not bound it.
    pub fn new(from: f32, to: f32) -> Self {
        Self {
            animation: Animation::new(AnimationConfig::new(Duration::from_secs(1))),
            from_value: from,
            to_value: to,
            stiffness: 200.0,
            damping: 20.0,
            mass: 1.0,
            velocity: 0.0,
            current_value: from,
        }
    }
    /// Returns a copy with the spring constant set to `s`: a higher value pulls
    /// the value toward the target harder, making the motion faster and bouncier.
    pub fn with_stiffness(mut self, s: f32) -> Self {
        self.stiffness = s;
        self
    }
    /// Returns a copy with the damping coefficient set to `d`: a higher value
    /// bleeds off velocity faster, reducing overshoot. `0.0` lets the spring
    /// oscillate indefinitely.
    pub fn with_damping(mut self, d: f32) -> Self {
        self.damping = d;
        self
    }
    /// Returns a copy with the moving mass set to `m`.
    ///
    /// Mass scales the acceleration (`a = F / m`), so a larger value slows every
    /// response. It must not be zero: the acceleration divides by it, so a zero
    /// mass yields a non-finite step.
    pub fn with_mass(mut self, m: f32) -> Self {
        self.mass = m;
        self
    }
    /// Starts the spring.
    ///
    /// The value is *not* reset to `from_value`; integration continues from
    /// wherever [`SpringAnimation::current_value`] currently stands.
    pub fn start(&mut self) {
        self.animation.start();
    }
    /// Stops the spring where it is, leaving the current value and velocity as
    /// they were.
    pub fn stop(&mut self) {
        self.animation.stop();
    }
    /// The value the spring was created from.
    ///
    /// This is informational: integration starts from `current_value`, so
    /// restarting a spring does not return it to `from_value`.
    pub fn from_value(&self) -> f32 {
        self.from_value
    }

    /// The value the spring has integrated to so far.
    pub fn current_value(&self) -> f32 {
        self.current_value
    }
    /// Reports whether the spring is running and not paused.
    pub fn is_running(&self) -> bool {
        self.animation.is_running()
    }

    /// Integrates the spring forward by one frame of `dt`.
    ///
    /// `dt` is clamped to at most 50 ms so a stalled or resumed application
    /// cannot take an oversized integration step. The spring is considered
    /// settled — and stops itself, snapping exactly onto `to_value` — once both
    /// the displacement and the velocity fall below `0.5`. This is a
    /// single-step Euler integration and is not unconditionally stable: a large
    /// `stiffness` combined with a small `mass` can diverge.
    pub fn update(&mut self, dt: Duration) {
        if !self.animation.is_running() {
            return;
        }
        let dt_secs = dt.as_secs_f32().min(0.05);
        // Spring physics: F = -kx - cv
        let displacement = self.current_value - self.to_value;
        let spring_force = -self.stiffness * displacement;
        let damping_force = -self.damping * self.velocity;
        let acceleration = (spring_force + damping_force) / self.mass;
        self.velocity += acceleration * dt_secs;
        self.current_value += self.velocity * dt_secs;
        // Check if settled
        if displacement.abs() < 0.5 && self.velocity.abs() < 0.5 {
            self.current_value = self.to_value;
            self.velocity = 0.0;
            self.animation.stop();
        }
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
        // Animation just started: should be RED or transitioning (r≥127, b≤127)
        assert!(color.r >= 127);
    }
    #[test]
    fn test_float_animation() {
        let config = AnimationConfig::new(Duration::from_millis(100));
        let mut animation = FloatAnimation::new(config, 0.0, 100.0);
        animation.start();
        let value = animation.current_value();
        assert!((0.0..=100.0).contains(&value));
    }
    #[test]
    fn animation_driver_add_and_advance() {
        let mut driver = AnimationDriver::new();
        let config = AnimationConfig::new(Duration::from_millis(100));
        let id = driver.add(config, |_| {});
        assert!(!driver.is_empty());
        driver.advance();
        driver.remove(id);
        assert!(driver.is_empty());
    }
    #[test]
    fn animation_driver_add_float() {
        let mut driver = AnimationDriver::new();
        let config = AnimationConfig::new(Duration::from_millis(100));
        let _id = driver.add_float(config, 10.0, 20.0, |_| {});
        driver.advance();
    }
    #[test]
    fn animation_driver_add_color() {
        let mut driver = AnimationDriver::new();
        let config = AnimationConfig::new(Duration::from_millis(100));
        let _id = driver.add_color(config, Color::RED, Color::BLUE, |_| {});
        driver.advance();
    }
    #[test]
    fn animation_driver_clear() {
        let mut driver = AnimationDriver::new();
        let c1 = AnimationConfig::new(Duration::from_millis(100));
        let c2 = AnimationConfig::new(Duration::from_millis(100));
        driver.add(c1, |_| {});
        driver.add(c2, |_| {});
        assert_eq!(driver.len(), 2);
        driver.clear();
        assert_eq!(driver.len(), 0);
    }
    #[test]
    fn animation_driver_is_empty() {
        let mut driver = AnimationDriver::new();
        assert!(driver.is_empty());
        let config = AnimationConfig::new(Duration::from_millis(100));
        driver.add(config, |_| {});
        assert!(!driver.is_empty());
        driver.clear();
        assert!(driver.is_empty());
    }
    #[test]
    fn animation_driver_default() {
        let driver: AnimationDriver = Default::default();
        assert!(driver.is_empty());
    }
    #[test]
    fn animation_driver_animate_named_property() {
        let mut driver = AnimationDriver::new();
        let id = driver.animate(
            "opacity",
            0.0,
            1.0,
            Duration::from_millis(100),
            EasingFunction::Linear,
            |_, _| {},
        );
        assert!(!driver.is_empty());
        // Verify that PropertyAnimation was wired up
        let prop_anim = driver.get_property_animation(id);
        assert!(prop_anim.is_some());
        let pa = prop_anim.unwrap();
        assert_eq!(pa.property, "opacity");
        assert!((pa.from - 0.0).abs() < 1e-6);
        assert!((pa.to - 1.0).abs() < 1e-6);
        driver.advance();
    }
    #[test]
    fn animation_driver_animate_linear() {
        let mut driver = AnimationDriver::new();
        let _id = driver.animate_linear("x", 10.0, 100.0, Duration::from_millis(100), |_, _| {});
        driver.advance();
    }
    #[test]
    fn animation_driver_animate_ease() {
        let mut driver = AnimationDriver::new();
        let _id = driver.animate_ease("width", 50.0, 200.0, Duration::from_millis(100), |_, _| {});
        driver.advance();
    }
    #[test]
    fn property_animation_struct_accessors() {
        let pa = PropertyAnimation {
            id: 42,
            property: "x".to_string(),
            from: 0.0,
            to: 100.0,
            current: 50.0,
        };
        assert_eq!(pa.id, 42);
        assert_eq!(pa.property, "x");
        assert!((pa.current - 50.0).abs() < 1e-6);
    }
    #[test]
    fn animation_driver_get_progress() {
        let mut driver = AnimationDriver::new();
        let id = driver.animate_linear("y", 0.0, 100.0, Duration::from_millis(100), move |_, _| {});
        // Initially should return Some(0.0) since animation just started
        let prog = driver.get_progress(id);
        assert!(prog.is_some());
    }
    #[test]
    fn parallel_animation_group() {
        let mut driver = AnimationDriver::new();
        let mut group = ParallelAnimation::new();
        let id1 = driver.animate_linear("a", 0.0, 1.0, Duration::from_millis(100), |_, _| {});
        let id2 = driver.animate_linear("b", 0.0, 1.0, Duration::from_millis(100), |_, _| {});
        group.add(id1);
        group.add(id2);
        assert_eq!(group.len(), 2);
        assert!(!group.is_completed(&driver));
        driver.advance();
    }
    #[test]
    fn sequential_animation_group() {
        let mut driver = AnimationDriver::new();
        let mut seq = SequentialAnimation::new();
        seq.add(AnimationConfig::new(Duration::from_millis(10)));
        seq.add(AnimationConfig::new(Duration::from_millis(10)));
        // Start the sequence
        let running = seq.advance(&mut driver, |_, _| {});
        assert!(running);
        driver.advance();
    }

    /// All easing functions should clamp to 0.0 for t < 0 and 1.0 for t > 1.
    #[test]
    fn test_easing_functions_clamp_range() {
        let variants = [
            EasingFunction::Linear,
            EasingFunction::EaseIn,
            EasingFunction::EaseOut,
            EasingFunction::EaseInOut,
            EasingFunction::BounceIn,
            EasingFunction::BounceOut,
            EasingFunction::ElasticIn,
            EasingFunction::ElasticOut,
            EasingFunction::BackIn,
            EasingFunction::BackOut,
        ];
        for variant in &variants {
            let below = variant.apply(-0.5);
            assert!(
                (below - 0.0).abs() < 1e-6,
                "{:?}.apply(-0.5) expected 0.0, got {}",
                variant,
                below
            );
            let above = variant.apply(1.5);
            assert!(
                (above - 1.0).abs() < 1e-6,
                "{:?}.apply(1.5) expected 1.0, got {}",
                variant,
                above
            );
        }
    }

    #[test]
    fn test_animation_driver_empty_advance() {
        let mut driver = AnimationDriver::new();
        // Advance on an empty driver should return 0 (no active animations).
        assert_eq!(driver.advance(), 0);
    }

    #[test]
    fn test_parallel_empty_is_completed() {
        let driver = AnimationDriver::new();
        let group = ParallelAnimation::new();
        // An empty parallel group should be considered completed.
        assert!(group.is_completed(&driver));
    }

    #[test]
    fn test_sequential_empty_stops_immediately() {
        let mut driver = AnimationDriver::new();
        let mut seq = SequentialAnimation::new();
        // An empty sequence should return false from advance() immediately.
        assert!(!seq.advance(&mut driver, |_, _| {}));
        assert!(!seq.advance(&mut driver, |_, _| {}));
    }
}
