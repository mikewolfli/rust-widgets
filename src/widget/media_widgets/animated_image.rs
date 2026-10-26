// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! AnimatedImage widget — plays caller-provided animation frame sequences.
//!
//! The widget manages a sequence of [`AnimatedFrame`]s (raw RGBA + delay) with
//! play/pause/stop controls, loop-count configuration, and signals when the
//! animation finishes or the current frame changes.
//!
//! **Decoding note**: GIF/APNG/WebP *stream* decoding is not built in. Decode
//! the container yourself (per frame) and hand the RGBA frames to
//! [`AnimatedImage::load_frames`], or install a provider with
//! [`AnimatedImage::set_frame_provider`] so the control fetches frames on demand.
//! The control deliberately exposes no byte-stream loader: a method that cannot
//! decode would only advertise a capability it does not have.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::effective_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

use super::pixels::nearest_scale;

/// A single frame of animation data.
pub struct AnimatedFrame {
    /// Raw RGBA pixel data for this frame (width * height * 4 bytes).
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Display duration for this frame in milliseconds.
    pub delay_ms: u64,
}

/// Image encoding format hint, used by a host's decoder.
///
/// # Host-decoding contract
///
/// This control ships no container decoder. The variants name the container a
/// host is expected to decode before calling [`AnimatedImage::load_frames`] or
/// installing an [`AnimatedFrameProvider`]; the control never parses these
/// bytes itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimatedImageFormat {
    /// Graphics Interchange Format.
    Gif,
    /// Animated PNG.
    Apng,
    /// Animated WebP.
    WebP,
}

/// Supplies decoded frames on demand instead of keeping every frame resident.
///
/// The control calls [`AnimatedFrameProvider::frame`] only for the frames it is about to show;
/// [`AnimatedFrameProvider::frame_count`] is the sequence length. A provider that cannot decode a
/// frame returns `None`, and the control then paints nothing for that index rather than inventing
/// a placeholder picture.
pub trait AnimatedFrameProvider {
    /// Number of frames in the sequence.
    fn frame_count(&self) -> usize;
    /// The decoded frame at `index`, or `None` when it cannot be produced.
    fn frame(&self, index: usize) -> Option<AnimatedFrame>;
}

/// How playback traverses the frame sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackDirection {
    /// Frames advance from the first to the last.
    #[default]
    Forward,
    /// Frames advance from the last to the first.
    Reverse,
}

/// One cached scaled frame, keyed by frame index, destination size and fit mode.
struct ScaledFrame {
    frame_index: usize,
    width: u32,
    height: u32,
    fit: AnimatedImageFit,
    data: Vec<u8>,
}

/// How a decoded frame is mapped into the control's rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimatedImageFit {
    /// Draw at the frame's own pixel size, centred and clipped (the historical behaviour).
    #[default]
    Original,
    /// Scale to fit inside the rectangle, preserving aspect ratio (letterbox).
    Contain,
    /// Scale to cover the rectangle, preserving aspect ratio and cropping the overflow.
    Cover,
    /// Stretch to the rectangle on both axes, without preserving aspect ratio.
    Fill,
}

/// AnimatedImage widget for playing animated image sequences.
pub struct AnimatedImage {
    base: BaseWidget,
    frames: Vec<AnimatedFrame>,
    current_frame: usize,
    playing: bool,
    /// Loop count: 0 = infinite, >0 = number of repetitions.
    loop_count: i32,
    /// Internal timer accumulator in milliseconds.
    frame_timer: u64,
    /// Frame delay in milliseconds (used when frame does not specify its own).
    frame_delay: u64,
    /// Number of completed loops so far.
    loops_completed: i32,
    /// Playback speed multiplier: `2.0` plays twice as fast, `0.5` half speed.
    speed: f32,
    /// How each frame is mapped into the control's rectangle.
    fit: AnimatedImageFit,
    /// Emitted when the animation finishes (all loops completed).
    pub animation_finished: GenericSignal,
    /// Emitted when the current frame index changes. Passes the new frame index.
    pub frame_changed: Signal1<u32>,
    /// How playback traverses the sequence.
    direction: PlaybackDirection,
    /// A host decoder that supplies frames on demand.
    frame_provider: Option<Box<dyn AnimatedFrameProvider>>,
    /// Frame count reported by the provider (zero when no provider is installed).
    provider_count: usize,
    /// Frames fetched from the provider, in least-recently-used-first order.
    provider_cache: Vec<(usize, AnimatedFrame)>,
    /// Upper bound on [`Self::provider_cache`] entries; `0` means unbounded.
    frame_cache_limit: usize,
    /// Cached result of the nearest-neighbour resample used by the fit modes.
    scale_cache: Option<ScaledFrame>,
    /// Per-frame delay overrides, `(frame index, delay in milliseconds)`.
    frame_delays: Vec<(usize, u64)>,
    /// The frame-counter caption, rebuilt only when the frame or the count changes.
    counter_text: String,
    /// The `(current, total)` pair `counter_text` was built from.
    counter_key: (usize, usize),
    /// Emitted with the new frame index after the animation advanced.
    pub frame_advanced: Signal1<u32>,
}

impl AnimatedImage {
    /// Creates a new AnimatedImage with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::AnimatedImage, geometry, "AnimatedImage"),
            frames: Vec::new(),
            current_frame: 0,
            playing: false,
            loop_count: 0,
            frame_timer: 0,
            frame_delay: 100,
            loops_completed: 0,
            speed: 1.0,
            fit: AnimatedImageFit::Original,
            animation_finished: GenericSignal::new(),
            frame_changed: Signal1::new(),
            direction: PlaybackDirection::Forward,
            frame_provider: None,
            provider_count: 0,
            provider_cache: Vec::new(),
            frame_cache_limit: 0,
            scale_cache: None,
            frame_delays: Vec::new(),
            counter_text: String::new(),
            counter_key: (usize::MAX, usize::MAX),
            frame_advanced: Signal1::new(),
        }
    }

    /// Replace the frame sequence with caller-decoded RGBA frames.
    ///
    /// Each frame carries its own `delay_ms`. Returns `Err` for an empty list.
    pub fn load_frames(&mut self, frames: Vec<AnimatedFrame>) -> Result<(), String> {
        if frames.is_empty() {
            return Err(format!(
                "an animated image needs at least one frame, got {}; pass the decoded frames \
                 from `decode_animation` instead of an empty list",
                frames.len()
            ));
        }
        self.frames = frames;
        self.frame_provider = None;
        self.provider_count = 0;
        self.provider_cache.clear();
        self.current_frame = 0;
        self.frame_timer = 0;
        self.loops_completed = 0;
        self.playing = false;
        self.scale_cache = None;
        self.frame_delays.clear();
        self.base.request_redraw();
        Ok(())
    }

    /// Installs a host decoder that supplies frames on demand.
    ///
    /// The control then keeps only the frames it has recently shown (bounded by
    /// [`Self::frame_cache_limit`]) instead of requiring every decoded frame up front. Installing a
    /// provider clears any eagerly loaded frames; `load_frames` conversely clears the provider, so
    /// exactly one of the two supplies the sequence.
    pub fn set_frame_provider(&mut self, provider: Box<dyn AnimatedFrameProvider>) {
        self.provider_count = provider.frame_count();
        self.frame_provider = Some(provider);
        self.frames.clear();
        self.provider_cache.clear();
        self.current_frame = 0;
        self.frame_timer = 0;
        self.loops_completed = 0;
        self.playing = false;
        self.scale_cache = None;
        self.frame_delays.clear();
        self.base.request_redraw();
    }

    /// Removes the host decoder, keeping any frames already fetched.
    pub fn clear_frame_provider(&mut self) {
        if self.frame_provider.take().is_none() {
            return;
        }
        self.provider_count = 0;
        self.provider_cache.clear();
        self.scale_cache = None;
        self.base.request_redraw();
    }

    /// Whether a host frame provider is installed.
    pub fn has_frame_provider(&self) -> bool {
        self.frame_provider.is_some()
    }

    /// The maximum number of provider frames kept resident; `0` means unbounded.
    pub fn frame_cache_limit(&self) -> usize {
        self.frame_cache_limit
    }

    /// Sets the provider-frame cache bound and evicts the least recently shown frames.
    pub fn set_frame_cache_limit(&mut self, limit: usize) {
        if self.frame_cache_limit == limit {
            return;
        }
        self.frame_cache_limit = limit;
        self.evict_provider_cache();
        self.base.request_redraw();
    }

    /// The resident RGBA bytes held by the control, in bytes.
    ///
    /// A host can use this to verify that a bounded provider cache really bounds memory rather
    /// than merely reporting that it should.
    pub fn memory_usage(&self) -> usize {
        let eager: usize = self.frames.iter().map(|frame| frame.data.len()).sum();
        let cached: usize = self.provider_cache.iter().map(|(_, frame)| frame.data.len()).sum();
        let scaled = self.scale_cache.as_ref().map(|cache| cache.data.len()).unwrap_or(0);
        eager + cached + scaled
    }

    /// The frame `index`, fetching it from the provider when it is not resident.
    ///
    /// Only a provider-backed sequence can fetch; an eager sequence answers from its own list.
    fn ensure_frame(&mut self, index: usize) -> Option<&AnimatedFrame> {
        if self.frame_provider.is_none() {
            return self.frames.get(index);
        }
        if let Some(position) = self.provider_cache.iter().position(|(slot, _)| *slot == index) {
            // Refresh recency so the least recently *shown* frame is the one evicted.
            if position + 1 != self.provider_cache.len() {
                let entry = self.provider_cache.remove(position);
                self.provider_cache.push(entry);
            }
            return self.provider_cache.last().map(|(_, frame)| frame);
        }
        let frame = self.frame_provider.as_ref()?.frame(index)?;
        self.provider_cache.push((index, frame));
        self.evict_provider_cache();
        self.provider_cache.iter().find(|(slot, _)| *slot == index).map(|(_, frame)| frame)
    }

    /// Drops cached provider frames beyond [`Self::frame_cache_limit`], never the current one.
    fn evict_provider_cache(&mut self) {
        if self.frame_cache_limit == 0 {
            return;
        }
        while self.provider_cache.len() > self.frame_cache_limit {
            let evictable = self
                .provider_cache
                .iter()
                .position(|(slot, _)| *slot != self.current_frame)
                .unwrap_or(0);
            self.provider_cache.remove(evictable);
        }
    }

    /// The resident frame `index`, without fetching it.
    fn cached_frame(&self, index: usize) -> Option<&AnimatedFrame> {
        if self.frame_provider.is_some() {
            self.provider_cache.iter().find(|(slot, _)| *slot == index).map(|(_, frame)| frame)
        } else {
            self.frames.get(index)
        }
    }

    /// Starts playback of the animation.
    pub fn play(&mut self) {
        if self.frame_count() == 0 {
            return;
        }
        self.playing = true;
        self.base.request_redraw();
    }

    /// Pauses playback, keeping the current frame visible.
    pub fn pause(&mut self) {
        self.playing = false;
        self.base.request_redraw();
    }

    /// Stops playback and resets to the first frame.
    pub fn stop(&mut self) {
        self.playing = false;
        self.current_frame = 0;
        self.frame_timer = 0;
        self.loops_completed = 0;
        self.base.request_redraw();
    }

    /// Returns whether the animation is currently playing.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Sets the loop count. 0 = infinite, >0 = number of repetitions.
    pub fn set_loop_count(&mut self, count: i32) {
        self.loop_count = count.max(0);
    }

    /// Returns the current loop count setting.
    pub fn loop_count(&self) -> i32 {
        self.loop_count
    }

    /// Returns the playback speed multiplier.
    pub fn speed(&self) -> f32 {
        self.speed
    }

    /// Sets the playback speed multiplier.
    ///
    /// The value is clamped to `0.05..=20.0`: a zero speed would divide the frame delay by zero,
    /// and an absurd maximum would make the animation a slide show with no frames left to show.
    pub fn set_speed(&mut self, speed: f32) {
        if !speed.is_finite() {
            return;
        }
        let speed = speed.clamp(0.05, 20.0);
        if self.speed == speed {
            return;
        }
        self.speed = speed;
        self.base.request_redraw();
    }

    /// Returns how frames are mapped into the control.
    pub fn fit(&self) -> AnimatedImageFit {
        self.fit
    }

    /// Sets how frames are mapped into the control.
    pub fn set_fit(&mut self, fit: AnimatedImageFit) {
        if self.fit == fit {
            return;
        }
        self.fit = fit;
        self.base.request_redraw();
    }

    /// Returns the current frame index.
    pub fn current_frame(&self) -> usize {
        self.current_frame
    }

    /// Selects the visible frame directly, resetting the frame timer.
    ///
    /// This is the seek entry point: a host scrubbing a timeline sets the index and receives the
    /// same `frame_changed` signal a normal advance produces. An index past the end is clamped; a
    /// sequence with no frames ignores the call.
    pub fn set_current_frame(&mut self, frame: usize) {
        let count = self.frame_count();
        if count == 0 {
            return;
        }
        let frame = frame.min(count - 1);
        if self.current_frame == frame {
            self.frame_timer = 0;
            return;
        }
        self.current_frame = frame;
        self.frame_timer = 0;
        self.frame_changed.emit(frame as u32);
        self.base.request_redraw();
    }

    /// How playback traverses the sequence.
    pub fn direction(&self) -> PlaybackDirection {
        self.direction
    }

    /// Sets the traversal direction. A change is visible on the next advance.
    pub fn set_direction(&mut self, direction: PlaybackDirection) {
        if self.direction == direction {
            return;
        }
        self.direction = direction;
        self.base.request_redraw();
    }

    /// Returns the total number of frames.
    pub fn frame_count(&self) -> usize {
        if self.frame_provider.is_some() {
            self.provider_count
        } else {
            self.frames.len()
        }
    }

    /// Advances to the next frame. Wraps around when reaching the end
    /// and handles loop counting.
    pub fn advance_frame(&mut self) {
        let count = self.frame_count();
        if count == 0 {
            return;
        }

        let stepped = match self.direction {
            PlaybackDirection::Forward => {
                let next = self.current_frame + 1;
                if next >= count {
                    None
                } else {
                    Some(next)
                }
            }
            PlaybackDirection::Reverse => {
                if self.current_frame == 0 {
                    None
                } else {
                    Some(self.current_frame - 1)
                }
            }
        };

        match stepped {
            Some(next) => self.current_frame = next,
            None => {
                // Reached the end of the sequence.
                if self.loop_count == 0 {
                    // Infinite looping: wrap around.
                    self.current_frame = match self.direction {
                        PlaybackDirection::Forward => 0,
                        PlaybackDirection::Reverse => count - 1,
                    };
                } else {
                    self.loops_completed += 1;
                    if self.loops_completed >= self.loop_count {
                        // All loops completed, stop.
                        self.playing = false;
                        self.animation_finished.emit();
                        // Stay on the last frame.
                        return;
                    }
                    self.current_frame = match self.direction {
                        PlaybackDirection::Forward => 0,
                        PlaybackDirection::Reverse => count - 1,
                    };
                }
            }
        }

        self.frame_timer = 0;
        self.frame_changed.emit(self.current_frame as u32);
        self.frame_advanced.emit(self.current_frame as u32);
        self.base.request_redraw();
    }

    /// Advances the animation timer by the given number of milliseconds.
    /// Returns true if the frame changed as a result.
    pub fn tick(&mut self, delta_ms: u64) -> bool {
        if !self.playing || self.frame_count() == 0 {
            return false;
        }

        // The current frame must be resident before its own delay is read: a provider-backed
        // sequence carries the duration on the frame it fetches.
        self.ensure_frame(self.current_frame);
        let delay = self.frame_delay_of(self.current_frame);

        // `saturating_add`: `frame_timer` is a `u64` fed by a public `tick(delta_ms)`, so a large
        // `delta_ms` (or many small ones between advances) could overflow it. A wrapped timer is
        // *smaller* than the delay, which would freeze the animation rather than advance it — the
        // sibling `lottie_widget` already saturates.
        self.frame_timer = self.frame_timer.saturating_add(delta_ms);
        // The speed multiplier is applied to the frame's own delay, so a 2x animation waits
        // half as long on every frame and a 0.5x one waits twice as long.
        let delay = (delay as f32 / self.speed).max(1.0) as u64;
        if self.frame_timer >= delay {
            self.advance_frame();
            true
        } else {
            false
        }
    }

    /// Resets the animation to the first frame without stopping.
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.frame_timer = 0;
        self.loops_completed = 0;
        self.frame_changed.emit(0);
        self.base.request_redraw();
    }

    /// Overrides the display duration of frame `index` in milliseconds.
    ///
    /// A variable-frame-rate asset (a GIF whose frames have different delays) can state each
    /// frame's own duration here; the override wins over the frame's own `delay_ms` and over
    /// [`Self::frame_delay`]. An index outside the sequence is ignored rather than stored, so a
    /// later reload cannot inherit a delay for a frame that no longer exists.
    pub fn set_frame_delay(&mut self, index: usize, delay_ms: u32) {
        if index >= self.frame_count() {
            return;
        }
        let delay = u64::from(delay_ms);
        match self.frame_delays.iter_mut().find(|(slot, _)| *slot == index) {
            Some(entry) => entry.1 = delay,
            None => self.frame_delays.push((index, delay)),
        }
        if index == self.current_frame {
            self.base.request_redraw();
        }
    }

    /// The display duration of frame `index`, in milliseconds.
    ///
    /// Resolution order: an explicit [`Self::set_frame_delay`] override, then the frame's own
    /// `delay_ms` when it declares one, then the control-wide default.
    pub fn frame_delay(&self, index: usize) -> u32 {
        self.frame_delay_of(index).min(u64::from(u32::MAX)) as u32
    }

    /// The delay the clock uses for `index`.
    fn frame_delay_of(&self, index: usize) -> u64 {
        if let Some((_, delay)) = self.frame_delays.iter().find(|(slot, _)| *slot == index) {
            return *delay;
        }
        self.cached_frame(index)
            .map(|frame| frame.delay_ms)
            .filter(|delay| *delay > 0)
            .unwrap_or(self.frame_delay)
    }

    /// The frame-counter caption, rebuilt only when its inputs changed.
    fn refresh_counter(&mut self, frame_index: usize, frame_count: usize) {
        if self.counter_key != (frame_index, frame_count) {
            self.counter_key = (frame_index, frame_count);
            self.counter_text.clear();
            use core::fmt::Write as _;
            let _ = write!(self.counter_text, "{}/{}", frame_index + 1, frame_count);
        }
    }
}

impl Widget for AnimatedImage {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(100, 100)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `AnimatedImage` implements `Draw`, so `Some(self)` is total and cannot be wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    // The trait contract takes `u32` milliseconds; this control's own `tick` takes `u64` so a
    // long animation cannot wrap its clock. Widening at the boundary keeps the trait uniform.
    fn tick(&mut self, delta_ms: u32) -> bool {
        AnimatedImage::tick(self, u64::from(delta_ms))
    }

    fn is_animating(&self) -> bool {
        self.is_playing()
    }
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "animation_finished" => {
                Some(EventSignalRef::unit("animation_finished", &self.animation_finished))
            }
            "frame_changed" => {
                Some(EventSignalRef::mapped("frame_changed", &self.frame_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "frame_advanced" => {
                Some(EventSignalRef::mapped("frame_advanced", &self.frame_advanced, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `AnimatedImage`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_media.in.rs` / `access_write_media.in.rs` dispatch: `playing` is
/// a boolean that maps onto `play` / `pause`.
impl WidgetProperties for AnimatedImage {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "playing" => Ok(CapabilityValue::Bool(self.is_playing())),
            "direction" => Ok(CapabilityValue::String(
                match self.direction() {
                    PlaybackDirection::Forward => "forward",
                    PlaybackDirection::Reverse => "reverse",
                }
                .to_string(),
            )),
            "frame_cache_limit" => Ok(CapabilityValue::UInt(self.frame_cache_limit() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "playing" => {
                if expect_bool(value)? {
                    self.play();
                } else {
                    self.pause();
                }
                Ok(())
            }
            "direction" => {
                let direction = match expect_string(value)?.to_ascii_lowercase().as_str() {
                    "forward" => PlaybackDirection::Forward,
                    "reverse" => PlaybackDirection::Reverse,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_direction(direction);
                Ok(())
            }
            "frame_cache_limit" => {
                self.set_frame_cache_limit(expect_usize(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["playing", "direction", "frame_cache_limit", BASE_PROPERTY_NAMES]
    }

    /// Runs the published playback commands.
    ///
    /// `set_playing` names no property: playback state is exposed as the read-only
    /// `playing`, and starting it is the control's own [`Self::play`]. The default
    /// `set_foo` convention would look for a `playing` property and, finding it but
    /// with no payload route, have told the caller to supply a value the property
    /// route then refuses. The command is real here, so an override is the honest
    /// shape.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "set_playing" => {
                self.play();
                Ok(())
            }
            _ => self.default_command(name),
        }
    }
}

impl Draw for AnimatedImage {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        if self.frame_count() == 0 {
            // Empty state: a neutral placeholder panel. The panel and the label are this
            // widget's chrome — there is no frame data here at all — so both follow the
            // theme instead of pinning the control to one appearance.
            let style = self.style();
            let themed = crate::style::resolved_theme_style_shared("animated_image");
            let base_bg = style
                .background_color
                .or_else(|| themed.as_ref().and_then(|resolved| resolved.background_color))
                .unwrap_or(Color::rgba(230, 230, 230, 200));
            let placeholder_text = style
                .text_color
                .or_else(|| themed.as_ref().and_then(|resolved| resolved.text_color))
                .unwrap_or(Color::rgba(160, 160, 160, 220));
            context.fill_rounded_rect(rect, 4, base_bg);
            let font = effective_font(style);
            let text = "No frames loaded";
            // Centred on the vertical midline and **fitted** to the control's width. The
            // label is 16 characters at 14 px, so it is wider than a control narrower than
            // 224 px; without the fit it ran past the panel and, in the SVG snapshot, past
            // the picture. The glyph origin is the glyph's top (`ascent` is inside the line
            // box, not above it), so half the height is the right centring offset.
            let metrics = context.measure_text(text, font);
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
            let line = Rect::new(rect.x, text_y, rect.width, metrics.height);
            context.draw_text_fitted_cow(
                line,
                text,
                font,
                placeholder_text,
                HorizontalAlignment::Center,
            );
            return;
        }

        let frame_count = self.frame_count();
        let frame_index = self.current_frame.min(frame_count - 1);
        // Everything that needs `&mut self` happens before the paint pass takes its style
        // borrow: fetching the frame, refreshing the cached caption and refreshing the
        // resample cache. The paint pass below only reads.
        self.ensure_frame(frame_index);
        self.refresh_counter(frame_index, frame_count);
        let frame_size = self.cached_frame(frame_index).map(|frame| (frame.width, frame.height));
        if let Some((fw, fh)) = frame_size {
            if let Some(fit) = self.fit_for_scaling() {
                let dest = fit_destination(rect, fw, fh, fit);
                let cached = matches!(
                    &self.scale_cache,
                    Some(cache)
                        if cache.frame_index == frame_index
                            && cache.width == dest.width
                            && cache.height == dest.height
                            && cache.fit == fit
                );
                if !cached {
                    let scaled = self
                        .cached_frame(frame_index)
                        .map(|frame| nearest_scale(&frame.data, fw, fh, dest.width, dest.height));
                    self.scale_cache = scaled.map(|data| ScaledFrame {
                        frame_index,
                        width: dest.width,
                        height: dest.height,
                        fit,
                        data,
                    });
                }
            }

            // Chrome colours resolve explicit style first, then the theme's resolved style for
            // this control, and only then fall back to the original literal. The literal step is
            // kept deliberately: an inactive theme must still give the widget a defined
            // appearance, and the value is the one this widget painted before, so an existing
            // pixel baseline cannot move. Borrowed once per draw.
            let style = self.style();
            let themed = crate::style::resolved_theme_style_shared("animated_image");
            let themed_bg = themed.as_ref().and_then(|resolved| resolved.background_color);
            let themed_border = themed.as_ref().and_then(|resolved| resolved.border_color);
            let base_bg =
                style.background_color.or(themed_bg).unwrap_or(Color::rgba(230, 230, 230, 200));
            // The page this shell sits in, read once. A disabled **fill** recedes toward the page
            // it sits on; it cannot recede toward its own contrast colour the way ink does,
            // because that would make it stand out *more* against the page.
            let window_fill = {
                let manager = crate::style::theme_manager();
                manager
                    .current_theme()
                    .map(|active| active.colors.background)
                    .unwrap_or(Color::WHITE)
            };
            let border_color =
                style.border_color.or(themed_border).unwrap_or(Color::rgba(160, 160, 160, 200));
            let font = effective_font(style);
            // Background — the surface the frame is matted onto, i.e. chrome. Disabled steps the
            // resolved fill toward its own contrast colour rather than toward a fixed white,
            // which on a dark surface made the disabled state *more* prominent than the enabled
            // one (1.13:1 against the window enabled, 3.63:1 disabled).
            let bg = if !is_enabled {
                self.base.disabled_surface_near(base_bg, window_fill)
            } else {
                base_bg
            };
            context.face_with_gradient_corners(
                rect,
                bg,
                style.background_gradient.as_ref(),
                style.surface.unwrap_or_default(),
                style.resolved_corner_radii().to_array(),
                Color::BLACK,
            );

            // Draw the frame, mapped into the control by the fit mode.
            let fw_i = fw as i32;
            let fh_i = fh as i32;
            context.push_clip(rect.x, rect.y, rect.width, rect.height);
            match self.fit {
                AnimatedImageFit::Original => {
                    let dx = rect.x + (rect.width as i32 - fw_i) / 2;
                    let dy = rect.y + (rect.height as i32 - fh_i) / 2;
                    if let Some(frame) = self.cached_frame(frame_index) {
                        context.draw_image(dx.max(0), dy.max(0), fw, fh, &frame.data);
                    }
                }
                fit => {
                    let dest = fit_destination(rect, fw, fh, fit);
                    if dest.width == fw && dest.height == fh {
                        if let Some(frame) = self.cached_frame(frame_index) {
                            context.draw_image(dest.x.max(0), dest.y.max(0), fw, fh, &frame.data);
                        }
                    } else if let Some(cache) = self.scale_cache.as_ref() {
                        context.draw_image(dest.x, dest.y, dest.width, dest.height, &cache.data);
                    }
                }
            }
            context.pop_clip();

            // Draw play/pause indicator overlay if animation is stopped or paused.
            if !self.playing {
                // The indicator chip is a status overlay rather than part of any frame, so the
                // theme sets its tone. Only the alpha is carried across: the badge must stay
                // dark enough to read as a scrim over arbitrary frame data.
                let overlay_color = border_color;
                let indicator_size = 32u32;
                let ix = rect.x + (rect.width as i32 - indicator_size as i32) / 2;
                let iy = rect.y + (rect.height as i32 - indicator_size as i32) / 2;
                let indicator_rect = Rect::new(ix, iy, indicator_size, indicator_size);
                context.fill_rounded_rect(indicator_rect, 6, overlay_color);

                if self.current_frame == 0 && !self.playing {
                    // Play triangle. Deliberately not themed: it is a media glyph drawn on the
                    // chip above, and a themed light-on-light pair would be invisible.
                    let cx = ix + indicator_size as i32 / 2;
                    let cy = iy + indicator_size as i32 / 2;
                    let tri_size = 10i32;
                    let points = [
                        Point::new(cx - tri_size / 2, cy - tri_size),
                        Point::new(cx - tri_size / 2, cy + tri_size),
                        Point::new(cx + tri_size / 2, cy),
                    ];
                    if let Some(first) = points.first() {
                        let mut prev = *first;
                        for p in points.iter().skip(1) {
                            context.draw_line(prev, *p, Color::WHITE);
                            prev = *p;
                        }
                        context.draw_line(prev, *first, Color::WHITE);
                    }
                }
            }

            // Frame counter overlay at top-right. The caption is cached by
            // `refresh_counter`; this path only borrows it.
            let counter_text = self.counter_text.as_str();
            let metrics = context.measure_text(counter_text, font);
            let cx = rect.x + rect.width as i32 - metrics.width as i32 - 4;
            let cy = rect.y + 2;
            let pill_w = metrics.width + 8;
            let pill_h = metrics.height + 2;
            let pill_rect = Rect::new(cx - 4, cy - 1, pill_w, pill_h);
            context.fill_rounded_rect(pill_rect, 3, border_color);
            context.draw_text(
                Point::new(cx, cy),
                counter_text,
                font,
                Color::WHITE,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl AnimatedImage {
    /// The fit mode that needs a resample, or `None` for the original-size fast path.
    fn fit_for_scaling(&self) -> Option<AnimatedImageFit> {
        match self.fit {
            AnimatedImageFit::Original => None,
            fit => Some(fit),
        }
    }
}

/// The destination rectangle a frame occupies under `fit`.
fn fit_destination(rect: Rect, width: u32, height: u32, fit: AnimatedImageFit) -> Rect {
    if width == 0 || height == 0 {
        return Rect::new(rect.x, rect.y, 0, 0);
    }
    match fit {
        AnimatedImageFit::Original => {
            let x = rect.x + (rect.width as i32 - width as i32) / 2;
            let y = rect.y + (rect.height as i32 - height as i32) / 2;
            Rect::new(x, y, width, height)
        }
        AnimatedImageFit::Fill => rect,
        AnimatedImageFit::Contain => {
            let scale = (rect.width as f32 / width as f32).min(rect.height as f32 / height as f32);
            let w = (width as f32 * scale).max(1.0) as u32;
            let h = (height as f32 * scale).max(1.0) as u32;
            Rect::new(
                rect.x + (rect.width as i32 - w as i32) / 2,
                rect.y + (rect.height as i32 - h as i32) / 2,
                w,
                h,
            )
        }
        AnimatedImageFit::Cover => {
            let scale = (rect.width as f32 / width as f32).max(rect.height as f32 / height as f32);
            let w = (width as f32 * scale).max(1.0) as u32;
            let h = (height as f32 * scale).max(1.0) as u32;
            // Cover crops the overflow, so the destination is centred on the rectangle and
            // `draw`'s clip is what removes the parts outside it.
            Rect::new(
                rect.x + (rect.width as i32 - w as i32) / 2,
                rect.y + (rect.height as i32 - h as i32) / 2,
                w,
                h,
            )
        }
    }
}

/// The destination rectangle a frame occupies under `fit`.
impl EventHandler for AnimatedImage {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            // Playback is toggled on **release**: a press/release pair used to flip the state
            // twice (playing -> paused -> playing), so a complete click looked like it did
            // nothing while still emitting two signals and two repaint requests.
            Event::MouseRelease { pos, button } => {
                if *button == 1 && self.geometry().contains_point(*pos) {
                    if self.playing {
                        self.pause();
                    } else {
                        self.play();
                    }
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Point;
    use std::sync::{Arc, Mutex};

    /// Two 2-frame sequences for playback/advance tests.
    fn two_test_frames() -> Vec<AnimatedFrame> {
        vec![
            AnimatedFrame { data: vec![10u8; 16], width: 2, height: 2, delay_ms: 100 },
            AnimatedFrame { data: vec![20u8; 16], width: 2, height: 2, delay_ms: 100 },
        ]
    }

    #[test]
    fn animated_image_creation_defaults() {
        let img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        assert_eq!(img.frame_count(), 0);
        assert_eq!(img.current_frame(), 0);
        assert!(!img.is_playing());
        assert_eq!(img.loop_count(), 0);
        assert_eq!(img.kind(), WidgetKind::AnimatedImage);
    }

    #[test]
    fn animated_image_load_frames_sets_sequence() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let frames = vec![
            AnimatedFrame { data: vec![0u8; 16], width: 2, height: 2, delay_ms: 100 },
            AnimatedFrame { data: vec![1u8; 16], width: 2, height: 2, delay_ms: 50 },
        ];
        img.load_frames(frames).unwrap();
        assert_eq!(img.frame_count(), 2);
        assert_eq!(img.current_frame(), 0);
    }

    #[test]
    fn animated_image_load_frames_rejects_empty() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        assert!(img.load_frames(Vec::new()).is_err());
    }

    #[test]
    fn a_frame_provider_supplies_only_the_frames_that_are_shown() {
        struct Provider {
            count: usize,
        }
        impl AnimatedFrameProvider for Provider {
            fn frame_count(&self) -> usize {
                self.count
            }
            fn frame(&self, index: usize) -> Option<AnimatedFrame> {
                Some(AnimatedFrame {
                    data: vec![index as u8; 16],
                    width: 2,
                    height: 2,
                    delay_ms: 100,
                })
            }
        }

        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        assert_eq!(img.memory_usage(), 0);
        img.set_frame_provider(Box::new(Provider { count: 4 }));
        assert!(img.has_frame_provider());
        assert_eq!(img.frame_count(), 4);

        img.set_frame_cache_limit(2);
        assert_eq!(img.frame_cache_limit(), 2);
        img.play();
        assert!(img.tick(100));
        assert!(img.tick(100));
        assert!(img.tick(100));
        assert_eq!(img.current_frame(), 3);
        assert!(
            img.memory_usage() <= 2 * 16,
            "a bounded cache must bound resident frames, got {}",
            img.memory_usage()
        );

        // The eager route still replaces the provider rather than coexisting with it.
        img.load_frames(two_test_frames()).unwrap();
        assert!(!img.has_frame_provider());
        assert_eq!(img.frame_count(), 2);
    }

    #[test]
    fn frame_seek_and_direction_are_honoured() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        img.load_frames(two_test_frames()).unwrap();
        assert_eq!(img.direction(), PlaybackDirection::Forward);
        img.set_current_frame(1);
        assert_eq!(img.current_frame(), 1);
        img.set_direction(PlaybackDirection::Reverse);
        img.play();
        assert!(img.tick(100));
        assert_eq!(img.current_frame(), 0, "reverse playback walks towards the first frame");
    }

    #[test]
    fn animated_image_play_pause_stop() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        let frames =
            vec![AnimatedFrame { data: vec![0u8; 16], width: 2, height: 2, delay_ms: 100 }];
        img.load_frames(frames).unwrap();

        assert!(!img.is_playing());
        img.play();
        assert!(img.is_playing());
        img.pause();
        assert!(!img.is_playing());
        img.play();
        assert!(img.is_playing());
        img.stop();
        assert!(!img.is_playing());
        assert_eq!(img.current_frame(), 0);
    }

    #[test]
    fn animated_image_advance_frame_wraps_with_infinite_loop() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        img.load_frames(two_test_frames()).unwrap();
        img.play();
        assert_eq!(img.frame_count(), 2);

        img.advance_frame();
        assert_eq!(img.current_frame(), 1);

        img.advance_frame();
        // With infinite loop (count=0), wraps back to 0.
        assert_eq!(img.current_frame(), 0);

        img.advance_frame();
        assert_eq!(img.current_frame(), 1);
    }

    #[test]
    fn animated_image_advance_frame_finite_loop() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        img.load_frames(two_test_frames()).unwrap();
        img.set_loop_count(2);
        img.play();
        assert_eq!(img.loops_completed, 0);

        // Frame 0 -> 1
        img.advance_frame();
        assert_eq!(img.current_frame(), 1);

        // Frame 1 -> wraps, loops_completed=1, frame=0
        img.advance_frame();
        assert_eq!(img.current_frame(), 0);
        assert_eq!(img.loops_completed, 1);

        // Frame 0 -> 1
        img.advance_frame();
        assert_eq!(img.current_frame(), 1);

        // Frame 1 -> wraps, loops_completed=2, stops (should not advance past last frame)
        img.advance_frame();
        assert_eq!(img.current_frame(), 1); // stays on last frame
        assert!(!img.is_playing());
    }

    #[test]
    fn animated_image_frame_changed_signal() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        img.load_frames(two_test_frames()).unwrap();
        img.play();

        let captured = Arc::new(Mutex::new(None));
        img.frame_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<u32>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        img.advance_frame();
        assert_eq!(*captured.lock().unwrap(), Some(1));
    }

    #[test]
    fn animated_image_animation_finished_signal() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        img.load_frames(two_test_frames()).unwrap();
        img.set_loop_count(1);
        img.play();

        let finished = Arc::new(Mutex::new(false));
        img.animation_finished.connect({
            let finished = Arc::clone(&finished);
            move || {
                *finished.lock().unwrap() = true;
            }
        });

        // Frame 0 -> 1 (first loop).
        img.advance_frame();
        // Frame 1 -> wraps: loops_completed reaches 1 -> finish.
        img.advance_frame();
        assert!(*finished.lock().unwrap());
    }

    #[test]
    fn animated_image_tick_advances_frame() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        img.load_frames(two_test_frames()).unwrap();
        img.play();

        let changed = img.tick(30); // below frame delay (100ms)
        assert!(!changed);
        assert_eq!(img.current_frame(), 0);

        let changed = img.tick(80); // total 110ms >= 100ms
        assert!(changed);
        assert_eq!(img.current_frame(), 1);
    }

    #[test]
    fn animated_image_tick_not_playing_does_nothing() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        img.load_frames(two_test_frames()).unwrap();
        // Not playing.
        let changed = img.tick(500);
        assert!(!changed);
        assert_eq!(img.current_frame(), 0);
    }

    #[test]
    fn animated_image_loop_count_get_set() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        assert_eq!(img.loop_count(), 0);
        img.set_loop_count(5);
        assert_eq!(img.loop_count(), 5);
        img.set_loop_count(-1); // should clamp to 0
        assert_eq!(img.loop_count(), 0);
    }

    #[test]
    fn animated_image_disabled_blocks_events() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        img.load_frames(two_test_frames()).unwrap();
        img.set_enabled(false);

        img.handle_event(&Event::MousePress { pos: Point::new(50, 50), button: 1, modifiers: 0 });
        assert!(!img.is_playing());
    }

    #[test]
    fn animated_image_reset() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 200, 200));
        img.load_frames(two_test_frames()).unwrap();
        img.play();
        img.advance_frame();
        assert_eq!(img.current_frame(), 1);

        img.reset();
        assert_eq!(img.current_frame(), 0);
        assert!(img.is_playing()); // still playing
    }

    #[test]
    fn animated_image_svg_output() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 100, 100));
        img.load_frames(vec![AnimatedFrame {
            data: vec![0u8; 4],
            width: 1,
            height: 1,
            delay_ms: 100,
        }])
        .unwrap();
        let svg = crate::widget::svg::render_to_svg(&mut img);
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("width=\"100\""));
        assert!(svg.ends_with("</svg>"));
    }

    /// N-V-06: `tick` with a huge `delta_ms` saturates rather than wrapping the frame timer.
    ///
    /// `frame_timer` is a `u64` fed by a public `tick`. A wrapping `+=` would make the timer
    /// *smaller* than the delay and freeze the animation instead of advancing it.
    #[test]
    fn a_huge_tick_does_not_wrap_the_frame_timer() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 100, 100));
        img.load_frames(crate::compat::vec![AnimatedFrame {
            width: 1,
            height: 1,
            data: crate::compat::vec![0u8; 4],
            delay_ms: 100,
        }])
        .unwrap();
        img.play();
        // Debug builds panic on the wrapping `+=`; reaching the assert is the check.
        let advanced = img.tick(u64::MAX);
        assert!(advanced, "a huge delta must advance the frame, not freeze it");
    }

    #[test]
    fn speed_scales_the_frame_delay() {
        let mut img = AnimatedImage::new(Rect::new(0, 0, 10, 10));
        img.load_frames(two_test_frames()).unwrap();
        img.set_speed(2.0);
        assert_eq!(img.speed(), 2.0);
        img.play();
        // The frame's own delay is 100 ms; at 2x, 50 ms is enough.
        assert!(img.tick(50));
        assert_eq!(img.current_frame(), 1);
    }

    #[test]
    fn a_fit_mode_maps_the_frame_into_a_contained_box() {
        let rect = Rect::new(0, 0, 200, 100);
        let dest = fit_destination(rect, 100, 100, AnimatedImageFit::Contain);
        assert_eq!(dest.height, 100);
        assert_eq!(dest.width, 100);
        assert_eq!(dest.x, 50, "contain letterboxes the frame");
        let cover = fit_destination(rect, 100, 100, AnimatedImageFit::Cover);
        assert!(cover.width >= rect.width, "cover fills the width and crops the rest");
    }
}
