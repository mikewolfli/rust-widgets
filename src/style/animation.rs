use crate::compat::HashMap;
use crate::core::Color;
use crate::style::theme_state::{StatefulTheme, WidgetState};
use std::time::{Duration, Instant};
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum EasingFunction {
    #[default]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationDirection {
    #[default]
    Normal,
    Reverse,
    Alternate,
    AlternateReverse,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationFillMode {
    #[default]
    None,
    Forwards,
    Backwards,
    Both,
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
    on_complete_callback: Option<Box<dyn FnMut()>>,
}
impl Animation {
    pub fn new(config: AnimationConfig) -> Self {
        Self {
            config,
            start_time: None,
            is_running: false,
            is_paused: false,
            current_iteration: 0,
            on_complete_callback: None,
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
    pub fn is_paused(&self) -> bool {
        self.is_paused
    }
    pub fn reset(&mut self) {
        self.is_running = false;
        self.is_paused = false;
        self.start_time = None;
        self.current_iteration = 0;
    }
    pub fn on_complete(&mut self, callback: Box<dyn FnMut()>) {
        self.on_complete_callback = Some(callback);
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
                    if let Some(mut callback) = self.on_complete_callback.take() {
                        callback();
                    }
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
        Self { animation: Animation::new(config), from_color: from, to_color: to }
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
        Self { animation: Animation::new(config), from_value: from, to_value: to }
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
    next_id: AnimationId,
}

impl AnimationDriver {
    /// Creates a new empty animation driver.
    pub fn new() -> Self {
        Self { animations: HashMap::new(), next_id: 1 }
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
        let _prop = property.into();
        let config = AnimationConfig::new(duration).with_easing(easing);
        let range = to - from;
        self.add(config, move |progress| {
            let value = from + range * progress;
            on_tick(value, progress);
        })
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
}

impl Default for AnimationDriver {
    fn default() -> Self {
        Self::new()
    }
}

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
    Some(driver.animate_linear(
        format!("state_{:?}_to_{:?}", from, to),
        0.0,
        1.0,
        duration,
        on_tick,
    ))
}

/// A group of animations that run concurrently.
pub struct ParallelAnimation {
    ids: Vec<AnimationId>,
}

impl ParallelAnimation {
    pub fn new() -> Self {
        Self { ids: Vec::new() }
    }
}

impl Default for ParallelAnimation {
    fn default() -> Self {
        Self::new()
    }
}

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

    pub fn is_empty(&self) -> bool {
        self.ids.is_empty()
    }
}

/// A group of animations that run sequentially (one after another).
pub struct SequentialAnimation {
    animations: Vec<AnimationConfig>,
    current_index: usize,
    current_id: Option<AnimationId>,
}

impl SequentialAnimation {
    pub fn new() -> Self {
        Self { animations: Vec::new(), current_index: 0, current_id: None }
    }
}

impl Default for SequentialAnimation {
    fn default() -> Self {
        Self::new()
    }
}

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
    pub fn new(property: impl Into<String>, duration: Duration, easing: EasingFunction) -> Self {
        Self {
            property: property.into(),
            config: AnimationConfig::new(duration).with_easing(easing),
        }
    }
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
    pub fn len(&self) -> usize {
        self.transitions.len()
    }
    pub fn is_empty(&self) -> bool {
        self.transitions.is_empty()
    }
    pub fn clear(&mut self) {
        self.transitions.clear();
    }
}

impl Default for TransitionManager {
    fn default() -> Self {
        Self::new()
    }
}

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
    pub fn with_stiffness(mut self, s: f32) -> Self {
        self.stiffness = s;
        self
    }
    pub fn with_damping(mut self, d: f32) -> Self {
        self.damping = d;
        self
    }
    pub fn with_mass(mut self, m: f32) -> Self {
        self.mass = m;
        self
    }
    pub fn start(&mut self) {
        self.animation.start();
    }
    pub fn stop(&mut self) {
        self.animation.stop();
    }
    pub fn from_value(&self) -> f32 {
        self.from_value
    }

    pub fn current_value(&self) -> f32 {
        self.current_value
    }
    pub fn is_running(&self) -> bool {
        self.animation.is_running()
    }

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
        let _id = driver.animate(
            "opacity",
            0.0,
            1.0,
            Duration::from_millis(100),
            EasingFunction::Linear,
            |_, _| {},
        );
        assert!(!driver.is_empty());
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
