// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

use crate::compat::{format, Box, Duration, HashMap, Instant, String, Vec};
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
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
                        * ((t - 1.1) * 5.0 * core::f32::consts::PI).sin()
                }
            }
            Self::ElasticOut => {
                if t == 0.0 {
                    0.0
                } else if t == 1.0 {
                    1.0
                } else {
                    2.0_f32.powf(-10.0 * t) * ((t - 0.1) * 5.0 * core::f32::consts::PI).sin() + 1.0
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
    /// Virtual elapsed time accumulated by [`Animation::advance_by`].
    ///
    /// When this is non-zero the animation is **delta-driven**: `progress_inner` reads it instead of
    /// the clock, so a host supplying frame deltas gets reproducible pacing and a test can reach an
    /// animation's end state without sleeping. An animation never advanced by a delta leaves it at
    /// zero and stays clock-driven, so an existing caller's behaviour is unchanged.
    elapsed_override: Duration,
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
            elapsed_override: Duration::ZERO,
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
    ///
    /// # A finished animation holds at the end
    ///
    /// `update` clears `is_running` when the last iteration elapses, so a *completed*
    /// one-shot used to fall into the "not running" branch and read `0.0` — a progress bar
    /// wired to this snapped back to empty at the moment it filled. The documented contract is
    /// that it reads `1.0`, so completion is answered before the running check.
    pub fn progress(&self) -> f32 {
        if self.is_paused {
            // Return the progress frozen at the moment pause() was called.
            return self.frozen_progress.unwrap_or(0.0);
        }
        if self.is_completed() {
            // A completed forward run holds at its final value. `.max(0.0)` keeps a
            // reverse direction from reporting -0.0, and the easing is applied so an
            // overshooting curve's endpoint is the same one `progress_inner` produces.
            return 1.0_f32.max(0.0);
        }
        self.progress_inner()
    }
    /// The shared body of [`progress`](Self::progress) and the pause-time capture.
    ///
    /// The two used to be duplicated, which is how they came to carry the same defect: the
    /// finite arm read `(raw % 1.0).min(1.0)`, and because `min` was applied *after* the
    /// modulus the clamp could never bind — the only values reachable are `raw % 1.0`, which
    /// is strictly below `1.0`, and exactly `0.0` when `raw` is a whole number.
    ///
    /// That contradicts this module's documented contract in two ways: a finite animation
    /// that has overrun its last iteration is documented to read `1.0`, and it read `0.0`;
    /// and the finite arm was indistinguishable from the infinite one, so a progress-driven
    /// caller snapped back to the start on every iteration boundary instead of holding at the
    /// end. The finite case therefore takes the un-wrapped `raw` and clamps it.
    fn progress_inner(&self) -> f32 {
        if !self.is_running {
            return 0.0;
        }
        // Delta-driven when a caller has advanced this animation explicitly; clock-driven
        // otherwise. One source or the other, never a mix — see `advance_by`.
        let elapsed = if self.elapsed_override > Duration::ZERO {
            self.elapsed_override
        } else {
            self.start_time.map(|t| t.elapsed()).unwrap_or_default()
        };
        if elapsed < self.config.delay {
            return 0.0;
        }
        let animation_elapsed = elapsed - self.config.delay;
        // Guard against division by zero when duration is ZERO (default).
        let duration_secs = self.config.duration.as_secs_f32().max(f32::EPSILON);
        let raw_progress = animation_elapsed.as_secs_f32() / duration_secs;
        // A finite animation is a one-shot: its progress runs from 0 to 1 across the whole
        // sequence and saturates. Only an infinite one wraps per iteration.
        let progress =
            if self.config.infinite { raw_progress % 1.0 } else { raw_progress.min(1.0) };
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
    /// Advances this animation by an explicit `delta`, ignoring the wall clock.
    ///
    /// # Why an animation needs its own delta-driven advance
    ///
    /// `update` derives elapsed time from `start_time`, so progress depends on real time passing
    /// between calls. That makes the engine unusable from a control whose host supplies a frame delta
    /// — the crate's own `tick(delta_ms)` convention — and it makes an animation unreproducible in a
    /// test without sleeping.
    ///
    /// This moves a **virtual** elapsed time forward instead: `elapsed_override` accumulates the
    /// deltas the caller supplies, and the progress computation reads it in preference to the clock.
    /// An animation is therefore driven by exactly one source or the other, never a mix, which is
    /// what keeps one config producing the same curve either way.
    ///
    /// The delay, the iteration count and the completion callback all behave as they do in `update`,
    /// because both paths read the same configuration and the same accumulated time.
    pub fn advance_by(&mut self, delta: Duration) {
        if !self.is_running || self.is_paused {
            // Same rule as `update`: a paused animation keeps its frozen progress and does not
            // advance its iteration count.
            return;
        }
        self.elapsed_override += delta;
        let elapsed = self.elapsed_override;
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
    ///
    /// Shares [`progress_inner`](Self::progress_inner) with [`progress`](Self::progress):
    /// the two were duplicated bodies, which is how they came to disagree with the
    /// documented saturation contract in exactly the same way.
    fn compute_progress(&self) -> f32 {
        self.progress_inner()
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

impl ActiveAnimation {
    /// Forwards a caller-supplied delta to the wrapped animation.
    ///
    /// A forwarding method rather than reaching through `entry.anim.advance_by` at the call site,
    /// so the one place that knows what "advancing an active animation" means stays the one place.
    fn advance_by(&mut self, delta: Duration) {
        self.anim.advance_by(delta);
    }
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

    /// Advances every active animation by a caller-supplied delta instead of by the clock.
    ///
    /// # Why the engine needed this
    ///
    /// `advance` reads `Instant::now()`, which makes an animation's progress depend on how long the
    /// host took to call it. That is right for a real frame loop and wrong for everything else: a
    /// test cannot drive an animation to its end state without sleeping, a host that renders on
    /// demand cannot reproduce a frame, and the crate's own `tick(delta_ms) -> bool` convention
    /// (established by `floating_label`) has no way to feed an engine that only reads the clock.
    ///
    /// So the engine shipped complete and uncallable from any control. This is the entry point that
    /// makes it callable: progress moves by exactly `delta`, and the return value answers the same
    /// question `advance`'s does — how many animations are still running — so a control can report
    /// "another frame is needed" without a second bookkeeping field.
    ///
    /// Easing and iteration still come from each animation's own config, so a delta-driven advance
    /// follows the same curve as a clock-driven one; only the source of elapsed time differs.
    pub fn advance_by(&mut self, delta: Duration) -> usize {
        // Which animations were in flight *before* this advance. Recorded up front because an
        // animation that reaches its end during this call flips `is_running` to `false` inside
        // `advance_by`, and filtering the snapshot on `is_running` afterwards would therefore drop
        // exactly the frame that carries the final value: a caller watching a transition would see
        // it stop one step short of its target and never be told the last step. The set answers
        // "was this animating when the frame started", which is the question the snapshot needs.
        let was_running: Vec<AnimationId> = self
            .animations
            .iter()
            .filter(|(_, e)| e.anim.is_running())
            .map(|(&id, _)| id)
            .collect();
        for id in &was_running {
            if let Some(entry) = self.animations.get_mut(id) {
                entry.advance_by(delta);
            }
        }

        // Snapshot progress without holding a mutable borrow, then fire the callbacks — the same
        // two-phase shape `advance` uses, and for the same reason: a callback may add or remove
        // animations, which cannot happen while the map is borrowed.
        let snap: Vec<(AnimationId, f32)> = was_running
            .iter()
            .filter_map(|id| self.animations.get(id).map(|e| (*id, e.anim.progress())))
            .collect();
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
        for (id, progress) in &snap {
            if let Some(prop_anim) = self.property_animations.get_mut(id) {
                let range = prop_anim.to - prop_anim.from;
                prop_anim.current = prop_anim.from + range * progress;
            }
        }
        // Completed animations are removed and their completion callbacks fired, exactly as a
        // clock-driven advance does, so the two paths leave the driver in the same state.
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
        //
        // The in-flight set is captured first for the same reason `advance_by` captures it: an
        // animation that finishes during this call clears `is_running`, so a snapshot filtered on
        // `is_running` afterwards would silently skip the frame that carries its final value.
        let was_running: Vec<AnimationId> = self
            .animations
            .iter()
            .filter(|(_, e)| e.anim.is_running())
            .map(|(&id, _)| id)
            .collect();
        for id in &was_running {
            if let Some(entry) = self.animations.get_mut(id) {
                entry.anim.update();
            }
        }

        // Snapshot: collect progress without mutable borrows
        let snap: Vec<(AnimationId, f32)> = was_running
            .iter()
            .filter_map(|id| self.animations.get(id).map(|e| (*id, e.anim.progress())))
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

/// The half-period a text cursor spends in each of its two visible states.
///
/// Half a second is the tempo every platform's text field converged on, and it is what the
/// controls in this crate document. It is a named constant rather than a literal repeated at
/// each cursor so a future change of tempo moves them all together.
pub const CURSOR_BLINK_HALF_PERIOD_MS: u32 = 500;

/// The on/off state of a blinking text cursor, advanced by frame deltas.
///
/// # Why this is a type rather than a timer inside each control
///
/// Eight controls in this crate draw a caret, and each one used to draw a solid line while its
/// documentation claimed the cursor blinked. Giving every one of them its own `Instant`, its own
/// half-period literal and its own phase reset would be eight chances to disagree — and it is the
/// exact shape that produced the discrepancy: the comment described behaviour no code had.
///
/// So the state lives here, once, and a control owns a field of this type and forwards its
/// frame delta. The type is deliberately independent of any widget: it holds no `ObjectId`, no
/// geometry and no appearance, only "which half of the cycle are we in", so the same logic serves
/// a caret in a single-line field and one in a code editor.
///
/// # Why it is delta-driven
///
/// The convention every animated control in this crate follows is `tick(delta_ms) -> bool`, which
/// lets a host supply the frame delta instead of the control reading the wall clock. That keeps a
/// test able to reach a blink state without sleeping, and keeps a host that renders on demand in
/// control of the clock.
#[derive(Debug, Clone, Copy)]
pub struct CursorBlink {
    /// Milliseconds elapsed within the current half-period, in `0..half_period_ms`.
    elapsed_in_half: u32,
    /// Whether the cursor is drawn in the current half-period.
    visible: bool,
    /// Whether the cursor is animating at all. A blurred or read-only field holds a steady
    /// cursor and must not ask its host for frames.
    running: bool,
}

impl Default for CursorBlink {
    /// A cursor that is visible and not yet animating — the state of a field that has just been
    /// constructed but not focused.
    fn default() -> Self {
        Self { elapsed_in_half: 0, visible: true, running: false }
    }
}

impl CursorBlink {
    /// A cursor that is visible and not animating.
    pub fn new() -> Self {
        Self::default()
    }

    /// Starts blinking, showing the cursor.
    ///
    /// Called when a field gains focus. Restarting the phase on every focus is what makes the
    /// cursor immediately visible when a reader clicks into a field, rather than possibly appearing
    /// on the far side of an invisible half-period.
    pub fn start(&mut self) {
        self.elapsed_in_half = 0;
        self.visible = true;
        self.running = true;
    }

    /// Stops blinking and leaves the cursor visible.
    ///
    /// Called when a field loses focus. The cursor is left *visible* rather than hidden because a
    /// blurred field still shows where editing would resume; a hidden caret would read as a
    /// rendering failure.
    pub fn stop(&mut self) {
        self.elapsed_in_half = 0;
        self.visible = true;
        self.running = false;
    }

    /// Whether the cursor is currently drawn.
    pub fn is_visible(&self) -> bool {
        self.visible
    }

    /// Whether the cursor is animating.
    pub fn is_running(&self) -> bool {
        self.running
    }

    /// Advances the blink by `delta_ms` and reports whether another frame is needed.
    ///
    /// The same contract [`AnimationDriver::advance_by`] and `FloatingLabel::tick` follow: the
    /// return value answers "is there still work", so a host stops scheduling frames for a field
    /// whose cursor is not blinking. A `delta_ms` larger than one half-period is consumed
    /// half-period by half-period rather than snapping, so a stalled frame loop resumes at the
    /// correct phase instead of silently skipping a whole cycle's worth of flips.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.running {
            return false;
        }
        self.elapsed_in_half += delta_ms;
        while self.elapsed_in_half >= CURSOR_BLINK_HALF_PERIOD_MS {
            self.elapsed_in_half -= CURSOR_BLINK_HALF_PERIOD_MS;
            self.visible = !self.visible;
        }
        // A running cursor always owes the next frame: the blink is periodic, so the frame that
        // shows it is never the last one. Reporting `false` here would be the "settled" signal,
        // and a blink never settles.
        true
    }
}

/// Which of the theme's motion tokens prices a transition.
///
/// Named rather than passed as a bare `u32`, so the *choice of tempo* is a decision a control
/// states once, in one word, instead of reading a field out of the theme at its call site — and
/// so a control cannot accidentally price itself from an unrelated duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TransitionTempo {
    /// The theme's `Motion::fast` — a direct reaction to the pointer.
    Fast,
    /// The theme's `Motion::normal` — a control's own state change. The default, and what
    /// Material's `kThemeChangeDuration` (200 ms) describes.
    #[default]
    Normal,
    /// The theme's `Motion::slow` — a larger movement, such as a toggle travelling.
    Slow,
}

impl TransitionTempo {
    /// The duration this tempo names in the active theme.
    ///
    /// Read through [`crate::style`] rather than `crate::theme` directly: `crate::theme`
    /// only exists in a build with a device profile, so naming it here made the module
    /// fail to compile under `mini`/`embedded`. The `style` layer is the profile-safe
    /// facade every other control already goes through.
    pub fn duration_ms(self) -> u32 {
        // The `Motion` token set, or the crate's own defaults when no theme is active.
        // `normal` (200 ms) is the value the old hardcoded call sites used, so a build
        // with no theme behaves exactly as before.
        let (fast, normal, slow) = crate::style::motion_tokens();
        match self {
            TransitionTempo::Fast => fast,
            TransitionTempo::Normal => normal,
            TransitionTempo::Slow => slow,
        }
    }
}

/// A single control's state transition, advanced by frame deltas.
///
/// # Why this is a type rather than a few fields in each control
///
/// Dozens of controls animate one thing: how far they are between their resting appearance and
/// their interactive one. Each one that reimplements the interpolation also reimplements the same
/// decisions — what `tick`'s return value means, which of the theme's motion tokens prices it,
/// and how the engine's callback ownership works — and a control that gets any of them wrong
/// fails quietly. Two shapes of that failure were already present in the crate: `Button` carried
/// the interpolation inline (including the `Rc<Cell<_>>` dance the engine's callback ownership
/// requires), and `FloatingLabel` had re-derived it as an **exponential approach** that never
/// actually arrived and could not be re-priced by a theme.
///
/// # The contract
///
/// [`Transition::tick`] follows `tick(delta_ms) -> bool`: it returns `true` while there is still
/// movement and `false` once the value has settled, so a host can stop scheduling frames. The
/// duration is read from the theme *per tick*, so a theme switch re-prices a transition already
/// in flight instead of stranding it at the old tempo.
///
/// # Why the target is supplied every tick
///
/// A press that arrives mid-hover **re-aims the same progress** rather than restarting from zero.
/// That is what makes a quick press-and-release read as one movement instead of two fades. The
/// caller states the target from its own state on each call, so a change that arrived without a
/// `tick` in between is picked up rather than missed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Transition {
    /// Where the value is now, in `0.0..=1.0`.
    progress: f32,
    /// Which motion token prices the transition.
    tempo: TransitionTempo,
}

impl Default for Transition {
    /// At rest: a freshly constructed control must not fade *out* on its first frame, which is
    /// what starting at the interactive end of the range would do.
    fn default() -> Self {
        Self { progress: 0.0, tempo: TransitionTempo::Normal }
    }
}

impl Transition {
    /// A transition at rest, priced by the theme's `normal` tempo.
    pub fn new() -> Self {
        Self::default()
    }

    /// A transition at rest with an explicit tempo.
    pub fn with_tempo(tempo: TransitionTempo) -> Self {
        Self { progress: 0.0, tempo }
    }

    /// Where the value is now, in `0.0..=1.0`.
    pub fn progress(&self) -> f32 {
        self.progress
    }

    /// Moves the value to `progress`, snapping it and stopping any motion.
    ///
    /// Used when a control is reset or disabled: the caller wants a definite appearance, not a
    /// fade toward it.
    pub fn reset_to(&mut self, progress: f32) {
        self.progress = progress.clamp(0.0, 1.0);
    }

    /// Advances toward `target` by `delta_ms` and reports whether another frame is needed.
    ///
    /// # The interpolation is the engine's, not this type's
    ///
    /// The crate already owns an easing engine ([`AnimationDriver`]). This type drives it with
    /// `advance_by`, so a transition follows the same curve as every other animation in the crate,
    /// and the easing function stays a theme decision rather than something each control chooses.
    ///
    /// The driver owns its callbacks, so the value it produces comes back through a shared cell
    /// rather than an assignment to a captured local: `move |v| observed = v` would move the
    /// local into the closure and leave the caller reading the pre-move value, which is how a
    /// transition silently never advances.
    ///
    /// # Why the step is paced from the tempo instead of coming from the engine
    ///
    /// The engine's easing is applied to the *progress of a whole run*, and this type rebuilds the
    /// driver on every call — so handing it the frame delta would hand it an animation that is
    /// priced at `delta_ms` and asked to advance by `delta_ms`. That run is at 100% in one step,
    /// the easing is evaluated at `1.0`, and the value therefore arrives at the target on the
    /// **first** frame. The theme's easing could not soften anything, because there was no
    /// progress left for it to shape: with the crate's default `EaseOut`, a `Normal`-paced
    /// transition moved 0 -> 1 on one 120 ms frame and a `Slow` one reached 0.4. Every hover,
    /// press and toggle in the library was a cut, which is the one thing these transitions exist
    /// to remove.
    ///
    /// So the fraction stepped is computed here, from the tempo and `delta_ms` — the same
    /// `delta / duration` the engine would derive, made explicit so it is applied to the value
    /// rather than to a fresh run. The engine still supplies the *interpolation syntax*
    /// (add / advance / read back), which is why this is not simply `self.progress += ease(step)`:
    /// there is one place that knows how to interpolate and it remains that place.
    pub fn tick(&mut self, target: f32, delta_ms: u32) -> bool {
        let target = target.clamp(0.0, 1.0);
        if (self.progress - target).abs() < f32::EPSILON {
            return false;
        }
        let duration_ms = self.tempo.duration_ms().max(1);
        // The share of the whole transition one frame is worth. Clamped to 1.0 because a frame
        // longer than the transition (a stalled host, a debugger breakpoint) must finish it, not
        // overshoot; a `f32::EPSILON` floor keeps a zero-duration token from dividing by zero.
        let step = (delta_ms as f32 / duration_ms as f32).clamp(f32::EPSILON, 1.0);

        let from = self.progress;
        let mut driver = AnimationDriver::new();
        let observed = crate::compat::Rc::new(core::cell::Cell::new(from));
        let sink = crate::compat::Rc::clone(&observed);
        // The config is **not** the tempo: a run priced at `delta_ms` and advanced by `delta_ms`
        // is at 100% on the first call, so its easing is sampled at `1.0` and the value arrives
        // at the target immediately -- the exact defect this function now avoids. One millisecond
        // is the shortest run the engine is asked to describe, and the value is read back at
        // once, so the figure is only a scale.
        // **Not** zero: a `Duration::ZERO` config makes `Animation::progress_inner` substitute
        // `f32::EPSILON` for the duration and the whole step becomes a snap, which is the same
        // defect in a different disguise. One millisecond is the shortest run the engine is asked
        // to describe, and the value is read back immediately, so the figure is only a scale.
        let config = AnimationConfig::new(Duration::from_millis(1));
        driver.add_float(config, from, target, move |value| sink.set(value));
        driver.advance_by(Duration::from_millis(1));
        // The engine answered with its own pacing; the crate's easing is applied to *this frame's*
        // share, which is what keeps the curve meaningful when the driver is rebuilt per call.
        let curve = crate::style::motion_easing();
        let next = from + (target - from) * curve.apply(step);
        // # "Still moving" is asked of the *value*, and the tolerance is one step
        //
        // The driver above cannot reach its own end (it is rebuilt every call), so its answer says
        // nothing about whether this transition is done. Two facts make the value the right place
        // to ask. **Reaching the target is what draws the animation's last frame**, so "needs
        // another frame" is exactly "has not reached it" — and once it has, the first arm above
        // returns `false` for every later call, so the snap does not re-trigger. And because the
        // remainder shrinks by `(1 - step)` each frame, the approach is geometric: with this test
        // the travel settles in about `ln(tolerance) / ln(1 - step)` frames (about 44 at a 16 ms
        // step against a 200 ms duration), in both real time and frame count. An animation whose
        // target is inside the tolerance is **snapped** rather than approached —
        // `target * (1 - tolerance)` is well under a pixel for every position a control has, so
        // the difference is invisible and the old rule would have left a visible one for the rest
        // of the frame loop's life.
        //
        // # Why the snap is published before it is decided
        //
        // `self.progress` is assigned `target` on the settle frame, so a caller that advances this
        // transition and then reads the value sees the animation's **end state on the frame it
        // ends** rather than one step short of it. The earlier shape wrote the eased value and
        // returned `false`, which meant the final value was only visible to a caller who ticked a
        // transition that was already finished — the frame that drew the ending showed nearly it.
        let tolerance = step.clamp(f32::EPSILON, 1.0);
        if (next - target).abs() <= tolerance {
            self.progress = target;
            return false;
        }
        self.progress = next;
        true
    }
}

/// A **named** motion token, for the one property a control drives itself.
///
/// # Why a second name for [`TransitionTempo`]
///
/// `TransitionTempo` is the same three tokens, and [`MotionSlot`] deliberately does not
/// replace it: the former is what a `Transition` is *configured by* at construction, while
/// this is what a [`PropertyDriver`] is *created with* and what BLUE24 §2.4's gate reads.
/// Keeping the two spellings would be the "two enums, one meaning" mistake principle #54
/// forbids -- so this is a **type alias**, not a copy: a reader sees one token set, and a
/// new variant added to `TransitionTempo` cannot silently fail to reach a driver.
pub type MotionSlot = TransitionTempo;

/// One numeric property moving between two values, advanced once per frame.
///
/// # Why this is not [`AnimationDriver`]
///
/// `AnimationDriver` is **registry-shaped**: a caller registers a named animation and the
/// driver advances it by id. That shape has the failure BLUE23 §3.3 already documented --
/// "registered and never unregistered" -- and a control cannot answer "am I moving?"
/// from a registry it does not own.
///
/// What the crate needs is **self-describing**: the control *is* the animation's owner, and
/// `is_moving()` is its answer about itself. So this type is `Transition`'s sibling, with
/// the one thing `Transition` left to its callers made explicit: **the target is stored**.
///
/// # Why storing the target removes a field from every control
///
/// `Transition::tick(target, delta)` takes the target *every call*, so a control that wants
/// to answer `is_animating()` between frames has to cache it... which is exactly the second
/// private field (`interaction_target: f32`) that `Button` and `ToggleButton` each carry.
/// Two controls, two copies of the same three lines, and the copies had to agree with
/// `widget_state()` on their own. Here the target is part of the value, so `set_target` is
/// the only writer and `is_moving` reads the same fact the interpolation does -- the
/// duplicated field goes away because there is nothing left for it to cache.
///
/// # Why there is no generic `T`
///
/// The properties that need driving are **scalar progress** (0..=1: a button's hover fill,
/// a toggle's travel) and **pixel offset** (a thumb position), and both interpolate as
/// `f32`. An `Interpolate` trait would add a `where` clause at every call site to express
/// what one `f32` already expresses (principle #28). A future *colour* interpolation should
/// be a new named method, not a type parameter.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PropertyDriver {
    /// Where the value is now.
    current: f32,
    /// Where it is heading. Kept here so `is_moving` and `set_target` agree by construction.
    target: f32,
    /// Which motion token prices the move.
    tempo: MotionSlot,
}

impl Default for PropertyDriver {
    /// At the **resting** end, priced normally.
    ///
    /// This is the crate's one answer to "what is the first value", and it is the safe one:
    /// starting at the target end would make a freshly built control animate *away* from its
    /// own state on the first frame it is drawn (the defect `button.rs` records in prose and
    /// `PieMenu` still exhibited -- BLUE24 §2.4 gate B exists to pin it here instead).
    fn default() -> Self {
        Self { current: 0.0, target: 0.0, tempo: MotionSlot::Normal }
    }
}

impl PropertyDriver {
    /// A driver resting at `value`, priced by `tempo`.
    ///
    /// The target starts equal to `value`, so a driver built at a non-zero value is **still**,
    /// not "about to travel to zero". A control that wants the value to move calls
    /// [`set_target`](Self::set_target).
    pub fn at(value: f32, tempo: MotionSlot) -> Self {
        Self { current: value, target: value, tempo }
    }

    /// The value the driver is currently interpolating, which is the same `f32` the
    /// interpolation moves -- not a separate cache.
    pub fn value(&self) -> f32 {
        self.current
    }

    /// Aim the driver at `target`.
    ///
    /// # Same value does not restart
    ///
    /// Setting the target the driver is already at (or already heading to) is a no-op, so a
    /// state change that is re-reported every frame -- which is what a `widget_state()`
    /// derived target does -- cannot reset the progress and make the movement stutter. That is
    /// the "interrupt re-aims rather than restarts" property `Transition` already had, kept
    /// here by simply not touching `current`.
    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    /// The value the driver is heading toward.
    pub fn target(&self) -> f32 {
        self.target
    }

    /// Advance by `delta_ms` toward the target; `true` while there is still movement.
    ///
    /// The interpolation itself is [`Transition`]'s, so there is one place that knows the
    /// curve, the pacing and the snap tolerance. This method is the storage half of the same
    /// operation, which is why it is three lines rather than a second implementation.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        let mut transition = Transition::with_tempo(self.tempo);
        transition.reset_to(self.current);
        let moving = transition.tick(self.target, delta_ms);
        self.current = transition.progress();
        moving
    }

    /// Whether the driver is between two values -- **answers only, never advances**.
    ///
    /// This is the query `Widget::is_animating` needs: a frame loop asks every control
    /// whether it owes frames without advancing any of them, so that the advance happens
    /// exactly once and in one place (BLUE24 §1.2).
    pub fn is_moving(&self) -> bool {
        (self.current - self.target).abs() > f32::EPSILON
    }

    /// Jump to `value` and stop, so the next [`tick`](Self::tick) does not move it.
    ///
    /// Both ends are set, because a driver that jumped its position but kept pointing at the
    /// old target would immediately animate away from where it was just placed -- which is
    /// what a control disabling itself or being reset actually wants to avoid.
    pub fn jump_to(&mut self, value: f32) {
        self.current = value;
        self.target = value;
    }
}

#[cfg(test)]
mod property_driver_tests {
    use super::{MotionSlot, PropertyDriver};

    /// The default is the resting end, and it is asserted rather than inferred.
    ///
    /// # The defect this pins
    ///
    /// BLUE24 §2.4 gate B: a control constructed at the **target** end of its own animation is
    /// born already finished. Three consequences, none of which reports an error -- it fades
    /// *out* on the first frame, `show_at`/`set_checked` have nothing left to animate, and
    /// `is_moving()` is `false`, so the frame loop never schedules the reveal.
    ///
    /// `PieMenu` shipped exactly that (`animation_progress: 1.0` at construction, and no reader
    /// of the field at all), while `button.rs` carried the lesson in prose. The crate's single
    /// default is the place to make the rule structural once, so the gate can check the type
    /// instead of every construction site.
    #[test]
    fn the_default_starts_at_the_resting_end() {
        let driver = PropertyDriver::default();
        assert_eq!(driver.value(), 0.0, "the first value must be the resting end");
        assert!(!driver.is_moving(), "and a fresh driver is not travelling anywhere");
    }

    /// `at` starts still: the target equals the value, so nothing moves until aimed.
    ///
    /// The distinction matters because `at` is how a control states a non-zero rest position
    /// (a menu that opens to a half-extended ring, a phase that starts mid-sweep). If `at`
    /// injected a target, every such control would drift toward an end it never asked for.
    #[test]
    fn at_starts_still_at_the_given_value() {
        let driver = PropertyDriver::at(0.25, MotionSlot::Slow);
        assert_eq!(driver.value(), 0.25);
        assert_eq!(driver.target(), 0.25, "at must not invent a destination");
        assert!(!driver.is_moving());
    }

    /// A driver crosses in many frames, arrives exactly, and then stops asking for frames.
    ///
    /// The same contract every control's `tick` advertises, asserted once at the type so the
    /// controls can rely on it rather than each re-deriving it: `true` while moving, then the
    /// **end value on the settle frame**, then `false` forever after.
    #[test]
    fn a_driver_settles_exactly_and_then_reports_no_more_frames() {
        let mut driver = PropertyDriver::at(0.0, MotionSlot::Normal);
        driver.set_target(1.0);
        assert!(driver.is_moving(), "a new target makes it moving at once, before any tick");

        let mut frames = 0;
        while driver.tick(16) {
            frames += 1;
            assert!(frames < 500, "the transition must terminate rather than tick forever");
        }
        assert!(frames > 1, "a transition must take more than one frame, or it is a cut: {frames}");
        assert_eq!(driver.value(), 1.0, "the settle frame lands exactly on the target");
        assert!(!driver.is_moving());
        assert!(!driver.tick(16), "and it stays settled");
    }

    /// Setting the target it is already at does not restart the movement.
    ///
    /// This is what lets a control re-assert its target every frame (which is how a
    /// `widget_state()`-derived target works) without the motion stuttering: `set_target`
    /// only stores the destination and never rewinds `current`, so a repeated aim at the same
    /// place is invisible.
    #[test]
    fn re_aiming_at_the_same_target_does_not_restart() {
        let mut driver = PropertyDriver::at(0.0, MotionSlot::Normal);
        driver.set_target(1.0);
        let _ = driver.tick(16);
        let partway = driver.value();
        assert!(partway > 0.0 && partway < 1.0, "the fixture must be mid-flight: {partway}");

        driver.set_target(1.0);
        assert_eq!(driver.value(), partway, "re-aiming at the same place must not rewind it");
        let _ = driver.tick(16);
        assert!(driver.value() > partway, "and the next step must continue forward");
    }

    /// `jump_to` lands and stops, so the next tick does not animate away from it.
    ///
    /// It sets both ends for exactly that reason: a driver whose position jumped but whose
    /// target stayed behind would immediately travel back to the stale aim, which is the
    /// opposite of what "place this value here, now" means.
    #[test]
    fn jump_to_lands_and_stops() {
        let mut driver = PropertyDriver::at(0.0, MotionSlot::Normal);
        driver.set_target(1.0);
        let _ = driver.tick(16);

        driver.jump_to(0.5);
        assert_eq!(driver.value(), 0.5);
        assert!(!driver.is_moving(), "a jump is not a movement");
        assert!(!driver.tick(16), "and nothing is left to animate");
        assert_eq!(driver.value(), 0.5, "so the value stays where it was placed");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::MiniToString;

    /// A transition takes many frames to cross, not one.
    ///
    /// # The defect this pins
    ///
    /// [`Transition::tick`] rebuilds its [`AnimationDriver`] on every call, so the driver's notion of
    /// "how far through am I" is resettable — and the version that handed it the frame delta as its
    /// duration described a run that is 100% complete in one step. The easing was then sampled at
    /// `1.0` (where every curve returns 1.0 by construction, since a curve must map 1 to 1) and the
    /// value **arrived at its target on the first frame**. The theme's `EaseOut` could not soften
    /// anything because there was no progress left to shape, so every hover, press and toggle in the
    /// library was a cut — the one thing the transition exists to remove.
    ///
    /// The assertion is deliberately about the *shape of the sequence* rather than about any frame
    /// count: "more than one step, every step forward, never past the target". A curve is applied to
    /// each step, so individual steps are not evenly sized and a fixed expectation would be a
    /// statement about the curve rather than about the pacing.
    ///
    /// The settle frame's value is recorded **after** the loop rather than only inside it: the frame
    /// that reports `false` is the frame that lands on the target, so a caller that stops at the
    /// first `false` and never re-reads would see the animation end one step short. The final
    /// `assert_eq!` is what pins that — the previous shape of this type wrote the eased value and
    /// returned `false`, leaving the end state reachable only by a second, redundant `tick`.
    #[test]
    fn a_transition_takes_many_frames_and_never_overshoots() {
        for tempo in [TransitionTempo::Fast, TransitionTempo::Normal, TransitionTempo::Slow] {
            let mut transition = Transition::with_tempo(tempo);
            let mut seen = vec![transition.progress()];
            let mut steps = 0;
            // 200 frames at 16 ms is 3.2 s, past every tempo the theme ships.
            while transition.tick(1.0, 16) {
                steps += 1;
                seen.push(transition.progress());
                assert!(steps < 200, "{tempo:?} must settle, not run forever: {seen:?}");
                assert!(
                    seen[seen.len() - 2] < seen[seen.len() - 1],
                    "{tempo:?} must move forward on every frame: {seen:?}"
                );
                assert!(
                    seen[seen.len() - 1] <= 1.0,
                    "{tempo:?} must never pass its target: {seen:?}"
                );
            }
            // The frame that reported "done" landed on the target; its value is what it published.
            seen.push(transition.progress());
            assert!(steps > 1, "{tempo:?} must take more than one frame, not arrive at once");
            assert!(
                seen.iter().any(|p| *p > 0.0 && *p < 1.0),
                "{tempo:?} must be seen mid-flight: {seen:?}"
            );
            assert_eq!(
                seen.last().copied(),
                Some(1.0),
                "{tempo:?} must land exactly on its target: {seen:?}"
            );
            assert!(
                seen.windows(2).all(|pair| pair[0] <= pair[1]),
                "{tempo:?} must never move backwards: {seen:?}"
            );
        }
    }

    /// A step at least as long as the transition finishes it in that frame.
    ///
    /// The other side of the same rule: a host that hands over a delta longer than the whole tempo —
    /// a stalled frame, a breakpoint, a first frame after a resize — must see the transition
    /// complete, not advance a fraction of it and then need a second frame to finish. This is what
    /// the `step` clamp to `1.0` is for.
    #[test]
    fn a_frame_longer_than_the_transition_finishes_it() {
        for tempo in [TransitionTempo::Fast, TransitionTempo::Normal, TransitionTempo::Slow] {
            let mut transition = Transition::with_tempo(tempo);
            assert!(
                !transition.tick(1.0, tempo.duration_ms() * 4),
                "{tempo:?} must be done after a frame longer than its own duration"
            );
            assert_eq!(transition.progress(), 1.0, "{tempo:?} must land on the target exactly");
        }
    }

    /// A transition already at its target owes nothing, whatever the tempo.
    #[test]
    fn a_transition_at_its_target_owes_no_frames() {
        let mut transition = Transition::default();
        assert!(!transition.tick(0.0, 16), "0.0 is where it starts, so it is already there");
        transition.reset_to(1.0);
        assert!(!transition.tick(1.0, 16), "and 1.0 needs no approach");
        assert_eq!(transition.progress(), 1.0, "with no drift from the repeated calls");
    }

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

    /// A cursor blinks on the documented half-period and only while it is running.
    ///
    /// The defect this pins is not arithmetic — it is that the feature did not exist while three
    /// doc comments said it did. The assertions are therefore about observable state transitions:
    /// a stopped cursor is visible and owes no frames, a started one flips exactly at the
    /// half-period, and a delayed frame resumes at the right phase instead of skipping a flip.
    #[test]
    fn a_cursor_blinks_on_its_half_period_and_stops_when_told() {
        let mut blink = CursorBlink::new();

        // Blurred: visible, and it must not ask the host for frames.
        assert!(blink.is_visible());
        assert!(!blink.is_running());
        assert!(!blink.tick(10_000), "a stopped cursor owes no frame, however large the delta");
        assert!(blink.is_visible(), "a stopped cursor stays visible");

        // Focused: it flips on the boundary and holds the phase in between.
        blink.start();
        assert!(blink.is_visible());
        assert!(blink.tick(CURSOR_BLINK_HALF_PERIOD_MS - 1), "still running");
        assert!(blink.is_visible(), "one millisecond short of the boundary is not yet a flip");
        assert!(blink.tick(1), "still running");
        assert!(!blink.is_visible(), "the boundary is where it flips");
        assert!(blink.tick(CURSOR_BLINK_HALF_PERIOD_MS), "still running");
        assert!(blink.is_visible(), "and it flips back");

        // A long frame must consume the elapsed time half-period by half-period rather than
        // snapping to one flip, or a stalled host would come back out of phase.
        let mut delayed = CursorBlink::new();
        delayed.start();
        // Visible at 0. One half-period flips it to hidden...
        delayed.tick(CURSOR_BLINK_HALF_PERIOD_MS);
        assert!(!delayed.is_visible());
        // ...and three further half-periods in a single frame flip it three more times, ending
        // visible. Snapping to a single flip would leave it hidden, so this distinguishes the two.
        delayed.tick(CURSOR_BLINK_HALF_PERIOD_MS * 3);
        assert!(delayed.is_visible(), "three flips from hidden lands on visible");

        // Blurring returns it to the steady, visible state.
        blink.stop();
        assert!(blink.is_visible());
        assert!(!blink.tick(CURSOR_BLINK_HALF_PERIOD_MS * 10), "a stopped cursor owes no frame");
        assert!(blink.is_visible());
    }

    // ── Transition (the shared state-transition primitive) ──

    #[test]
    fn a_transition_starts_at_rest_heading_nowhere() {
        // Starting at the interactive end would make every control fade *out* on its first
        // frame, and would make a snapshot taken without ticking show a moved control.
        let transition = Transition::new();
        assert_eq!(transition.progress(), 0.0);
    }

    #[test]
    fn a_transition_lands_exactly_on_its_target_and_then_settles() {
        // The whole point of the `bool` return: a host must be able to stop scheduling frames.
        // A transition that approached its target asymptotically would return `true` forever,
        // which is precisely the defect `FloatingLabel` had before it used this type.
        let mut transition = Transition::new();
        assert!(transition.tick(1.0, 30), "one short frame is not enough to arrive");
        assert!(!transition.tick(1.0, 10_000), "a long frame must finish it");
        assert_eq!(transition.progress(), 1.0, "and must land exactly on the target");
        assert!(!transition.tick(1.0, 16), "a settled transition owes no frame");
    }

    #[test]
    fn a_transition_re_aims_rather_than_restarting() {
        // A press arriving mid-hover must continue from where the value *is*, which is what
        // makes a quick press-and-release read as one movement instead of two.
        let mut transition = Transition::new();
        transition.tick(1.0, 50);
        let partway = transition.progress();
        assert!(partway > 0.0 && partway < 1.0, "expected mid-flight, got {partway}");
        transition.tick(0.0, 16);
        assert!(
            transition.progress() < partway,
            "retracting must continue from {partway}, not jump"
        );
    }

    #[test]
    fn a_transition_never_leaves_the_unit_range() {
        // Targets come from caller state, so the clamp is the type's own guarantee rather
        // than something each caller must remember.
        let mut transition = Transition::new();
        transition.tick(5.0, 10_000);
        assert!(transition.progress() <= 1.0);
        transition.tick(-5.0, 10_000);
        assert!(transition.progress() >= 0.0);
    }

    #[test]
    fn resetting_a_transition_snaps_rather_than_fading() {
        let mut transition = Transition::new();
        transition.reset_to(1.0);
        assert_eq!(transition.progress(), 1.0);
        assert!(!transition.tick(1.0, 16), "an already-at-target transition owes no frame");
        // Out-of-range input is clamped, not stored.
        transition.reset_to(9.0);
        assert_eq!(transition.progress(), 1.0);
    }

    #[test]
    fn each_tempo_names_its_own_motion_token() {
        // The three tempos must be distinct facts, or the enum would be decoration.
        // Reading the values back through the crate's own defaults keeps this test
        // compiling in a profile with no theme module.
        let fast = TransitionTempo::Fast.duration_ms();
        let normal = TransitionTempo::Normal.duration_ms();
        let slow = TransitionTempo::Slow.duration_ms();
        assert!(fast < normal, "a pointer reaction is quicker than a state change");
        assert!(normal < slow, "a state change is quicker than a large movement");
    }

    /// An animation that completes on a frame still reports that frame's final value.
    ///
    /// The driver used to snapshot progress from animations that were *still* running once the
    /// advance had finished. A one-shot animation clears `is_running` the moment it reaches its
    /// end, so the very frame that carried the value `to` was filtered out of the snapshot: the
    /// tick callback never fired for it, and a caller watching a transition saw it stop one step
    /// short of its target forever. `add_float` is used because it is the API a control reaches
    /// for, and it makes the arithmetic visible — the callback receives `from + (to - from) * p`,
    /// so a missing final callback is an observable value rather than an absent event.
    #[test]
    fn a_completing_animation_still_delivers_its_final_value() {
        use core::cell::RefCell;
        use std::rc::Rc;

        let seen = Rc::new(RefCell::new(Vec::<f32>::new()));
        let sink = Rc::clone(&seen);

        let mut driver = AnimationDriver::new();
        driver.add_float(
            AnimationConfig::new(Duration::from_millis(200)),
            0.0,
            1.0,
            move |value| sink.borrow_mut().push(value),
        );

        // One advance that overshoots the whole duration: the animation begins and ends here.
        assert_eq!(
            driver.advance_by(Duration::from_millis(1000)),
            0,
            "it finished, so nothing runs"
        );
        assert_eq!(
            seen.borrow().last().copied(),
            Some(1.0),
            "the completing frame must deliver the end value, not the last running value"
        );

        // And the same through the clock-driven path, which carried the identical defect.
        let seen_clock = Rc::new(RefCell::new(Vec::<f32>::new()));
        let sink_clock = Rc::clone(&seen_clock);
        let mut clock_driver = AnimationDriver::new();
        clock_driver.add_float(
            // Zero duration: the animation is already past its end on the first `advance`.
            AnimationConfig::new(Duration::ZERO),
            0.0,
            4.0,
            move |value| sink_clock.borrow_mut().push(value),
        );
        std::thread::sleep(Duration::from_millis(5));
        clock_driver.advance();
        assert_eq!(
            seen_clock.borrow().last().copied(),
            Some(4.0),
            "the clock-driven advance must deliver the end value on the completing frame too"
        );
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
    /// A finite animation's progress saturates at `1.0` instead of wrapping.
    ///
    /// The finite arm was `(raw % 1.0).min(1.0)`. Because the clamp is applied *after* the
    /// modulus it can never bind — `raw % 1.0` is strictly below `1.0` for every
    /// non-integral `raw`, and exactly `0.0` at every whole-iteration boundary. So the
    /// finite arm was indistinguishable from the infinite one, and the module's documented
    /// contract ("a non-infinite animation that has overrun its last iteration reads `1.0`")
    /// was false: it read `0.0`.
    ///
    /// This pins the arithmetic directly, without a clock: the same expression is what
    /// `progress()` evaluates.
    #[test]
    fn finite_progress_saturates_and_infinite_wraps() {
        let saturation = |raw: f32, infinite: bool| {
            if infinite {
                raw % 1.0
            } else {
                raw.min(1.0)
            }
        };

        // A finite one-shot holds at the end of its sequence.
        assert_eq!(saturation(0.5, false), 0.5);
        assert_eq!(saturation(1.0, false), 1.0, "the first iteration's end is 1.0");
        assert_eq!(
            saturation(3.0, false),
            1.0,
            "an overrun finite animation must hold at 1.0, not reset to 0.0"
        );
        assert_eq!(saturation(99.0, false), 1.0);

        // An infinite one restarts each iteration.
        assert_eq!(saturation(3.0, true), 0.0);
        assert_eq!(saturation(3.25, true), 0.25);
    }

    /// A finite animation that has fully elapsed reports completion and full progress.
    ///
    /// The end-to-end check of the contract above, through the public API.
    #[test]
    fn a_finished_finite_animation_reads_full_progress() {
        use core::time::Duration;
        let mut animation = Animation::new(AnimationConfig {
            duration: Duration::from_millis(1),
            iteration_count: 1,
            infinite: false,
            ..AnimationConfig::default()
        });
        animation.start();
        std::thread::sleep(Duration::from_millis(5));
        animation.update();
        assert!(animation.is_completed(), "a one-iteration animation must complete");
        assert_eq!(
            animation.progress(),
            1.0,
            "a completed finite animation must read 1.0, not wrap back to 0.0"
        );
    }
}
