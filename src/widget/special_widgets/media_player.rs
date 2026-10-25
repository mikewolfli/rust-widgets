// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MediaPlayer widget.

use core::fmt::Write as _;

use crate::core::Point;
use crate::core::{Color, HorizontalAlignment, MediaClock, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use alloc::sync::Arc;

/// The inset a media player's own chrome keeps from its surface edges: 10.
///
/// One value for the two stacked labels and the transport rule, so all three share a leading
/// edge and a bottom margin instead of each spelling its own `10`/`6`/`8` offsets.
const MEDIA_PLAYER_PADDING: i32 = 10;

/// Lightweight media player stateful control.
/// A decoded video frame, ready to paint.
///
/// Pixels are RGBA8 in row-major order and are shared, so a decoder can hand the same buffer
/// to several players without copying it. `MediaPlayer` scales the frame into its picture area,
/// preserving the frame's aspect ratio.
#[derive(Clone, Debug)]
pub struct MediaFrame {
    pixels: Arc<Vec<u8>>,
    width: u32,
    height: u32,
}

impl MediaFrame {
    /// Wraps `pixels` as a `width`×`height` RGBA frame, or `None` when the buffer is too short.
    pub fn new(width: u32, height: u32, pixels: Vec<u8>) -> Option<Self> {
        let expected = width as usize * height as usize * 4;
        if width == 0 || height == 0 || pixels.len() < expected {
            return None;
        }
        Some(Self { pixels: Arc::new(pixels), width, height })
    }

    /// Wraps an already shared buffer.
    pub fn from_shared(width: u32, height: u32, pixels: Arc<Vec<u8>>) -> Option<Self> {
        let expected = width as usize * height as usize * 4;
        (width > 0 && height > 0 && pixels.len() >= expected).then_some(Self {
            pixels,
            width,
            height,
        })
    }

    /// The frame width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// The frame height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// The shared RGBA bytes.
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
}

/// A media transport: source, clock, transport rule, volume, rate and optional picture.
///
/// The control owns the interaction and the display; decoding and process handling stay with
/// the host, which feeds frames through [`MediaPlayer::set_frame`].
pub struct MediaPlayer {
    base: BaseWidget,
    source: Option<String>,
    /// The master timeline (BLUE24 §12 U-2).
    ///
    /// # Why the position moved out of this struct
    ///
    /// This control used to hold `playing: bool` and `position_ms: u64` of its own, and advance the
    /// second with `tick` — which is exactly the same fact as a [`MediaClock`]'s state and position,
    /// written a second time. Two copies of one timeline is the defect principle #54 names: the day
    /// one of them gained a rate, a seek clamp or an end rule, the other would not. The clock is now
    /// the single home for the position and the state, and this control reads them through it.
    ///
    /// The public API is unchanged — `position_ms()`, `is_playing()`, `play()`, `pause()`, `seek_to()`
    /// all still answer as before — so a caller sees a refactor rather than a behaviour change.
    clock: MediaClock,
    /// The media's duration, in milliseconds.
    ///
    /// Kept as the control's own field rather than read from the clock because `duration_ms` is a
    /// **uint** in the capability contract (`duration_ms`), while the clock works in seconds. The
    /// clock mirrors it for its clamp; this is the value the property reports.
    duration_ms: u64,
    volume: u8,
    muted: bool,
    fullscreen: bool,
    /// The playback rate multiplier.
    rate: f32,
    /// The decoded frame to show, if any.
    frame: Option<MediaFrame>,
    /// Whether the progress handle is being dragged.
    scrubbing: bool,
    /// Whether the volume rail is being dragged.
    adjusting_volume: bool,
    /// Media queued after the current one.
    playlist: Vec<String>,
    /// The playlist entry currently loaded, when the source came from the playlist.
    playlist_index: Option<usize>,
    /// The chapter markers, as `(position_ms, title)`.
    chapters: Vec<(u64, String)>,
    /// Emitted when the host should change the window's fullscreen state.
    pub fullscreen_requested: Signal1<bool>,
    /// Emitted when playback toggles.
    pub playback_changed: Signal1<bool>,
    /// Emitted when position changes.
    pub position_changed: Signal1<u64>,
    /// Emitted when volume changes.
    pub volume_changed: Signal1<u8>,
    /// Emitted when source changes.
    pub source_changed: Signal1<String>,
    /// Emitted exactly once when playback reaches the media's end.
    pub media_ended: GenericSignal,
    /// Emitted with the chapter index when a chapter marker is pressed.
    pub chapter_selected: Signal1<usize>,
}

impl MediaPlayer {
    /// Creates empty media player.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebEngineView, geometry, "MediaPlayer"),
            source: None,
            clock: MediaClock::new(),
            duration_ms: 0,
            volume: 80,
            muted: false,
            fullscreen: false,
            rate: 1.0,
            frame: None,
            scrubbing: false,
            adjusting_volume: false,
            playlist: Vec::new(),
            playlist_index: None,
            chapters: Vec::new(),
            fullscreen_requested: Signal1::new(),
            playback_changed: Signal1::new(),
            position_changed: Signal1::new(),
            volume_changed: Signal1::new(),
            source_changed: Signal1::new(),
            media_ended: GenericSignal::new(),
            chapter_selected: Signal1::new(),
        }
    }

    /// The master timeline, so a caller can read the rate or ask for a frame verdict.
    pub fn clock(&self) -> &MediaClock {
        &self.clock
    }

    /// The master timeline, mutably — for the playback rate.
    pub fn clock_mut(&mut self) -> &mut MediaClock {
        &mut self.clock
    }

    /// Whether playback is running. Named for what the clock reports, so the two cannot disagree.
    fn is_playing_internal(&self) -> bool {
        self.clock.state() == crate::core::PlaybackState::Playing
    }

    /// Returns current media source.
    pub fn source(&self) -> Option<&str> {
        self.source.as_deref()
    }

    /// Sets media source and optional known duration.
    pub fn set_source(&mut self, source: impl Into<String>, duration_ms: u64) {
        let source = source.into();
        self.source = Some(source.clone());
        self.duration_ms = duration_ms;
        self.clock.stop();
        self.clock.set_duration(duration_ms as f64 / 1000.0);
        self.source_changed.emit(source);
        self.playback_changed.emit(false);
        self.position_changed.emit(0);
        self.base.request_redraw();
    }

    /// Clears loaded source.
    pub fn clear_source(&mut self) {
        self.source = None;
        self.duration_ms = 0;
        self.clock.stop();
        self.clock.set_duration(0.0);
        self.playback_changed.emit(false);
        self.position_changed.emit(0);
        self.base.request_redraw();
    }

    /// Returns whether playback is active.
    pub fn is_playing(&self) -> bool {
        self.is_playing_internal()
    }

    /// Returns duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Returns current playback position in milliseconds.
    ///
    /// Read through the clock rather than from a field of its own: one timeline, one answer.
    pub fn position_ms(&self) -> u64 {
        (self.clock.position() * 1000.0).round() as u64
    }

    /// Returns volume level [0, 100].
    pub fn volume(&self) -> u8 {
        self.volume
    }

    /// Returns muted state.
    pub fn muted(&self) -> bool {
        self.muted
    }

    /// Returns fullscreen state.
    pub fn fullscreen(&self) -> bool {
        self.fullscreen
    }

    /// Starts playback if source exists.
    pub fn play(&mut self) -> bool {
        if self.source.is_none() {
            return false;
        }
        if !self.is_playing_internal() {
            self.clock.play();
            self.playback_changed.emit(true);
            self.base.request_redraw();
        }
        true
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        if self.is_playing_internal() {
            self.clock.pause();
            self.playback_changed.emit(false);
            self.base.request_redraw();
        }
    }

    /// Toggles play/pause.
    pub fn toggle_playback(&mut self) -> bool {
        if self.is_playing_internal() {
            self.pause();
            true
        } else {
            self.play()
        }
    }

    /// Seeks to absolute position.
    pub fn seek_to(&mut self, position_ms: u64) {
        let next = position_ms.min(self.duration_ms);
        if next != self.position_ms() {
            self.clock.seek(next as f64 / 1000.0);
            self.position_changed.emit(self.position_ms());
            self.base.request_redraw();
        }
    }

    /// Seeks by signed delta milliseconds.
    pub fn seek_by(&mut self, delta_ms: i64) {
        let current = self.position_ms() as i64;
        let max_pos = self.duration_ms as i64;
        let next = (current + delta_ms).clamp(0, max_pos);
        self.seek_to(next as u64);
    }

    /// Sets volume [0, 100].
    pub fn set_volume(&mut self, volume: u8) {
        let next = volume.min(100);
        if next != self.volume {
            self.volume = next;
            self.volume_changed.emit(self.volume);
            self.base.request_redraw();
        }
    }

    /// Sets muted state.
    pub fn set_muted(&mut self, muted: bool) {
        if self.muted != muted {
            self.muted = muted;
            self.base.request_redraw();
        }
    }

    /// Toggles mute.
    pub fn toggle_mute(&mut self) {
        self.set_muted(!self.muted);
    }

    /// Sets fullscreen state.
    pub fn set_fullscreen(&mut self, fullscreen: bool) {
        if self.fullscreen != fullscreen {
            self.fullscreen = fullscreen;
            self.fullscreen_requested.emit(fullscreen);
            self.base.request_redraw();
        }
    }

    /// Toggles fullscreen state.
    pub fn toggle_fullscreen(&mut self) {
        self.set_fullscreen(!self.fullscreen);
    }

    /// Returns the playback rate multiplier.
    pub fn playback_rate(&self) -> f32 {
        self.rate
    }

    /// Sets the playback rate, clamped to `0.25..=4.0`.
    pub fn set_playback_rate(&mut self, rate: f32) {
        if !rate.is_finite() {
            return;
        }
        let rate = rate.clamp(0.25, 4.0);
        if (self.rate - rate).abs() < f32::EPSILON {
            return;
        }
        self.rate = rate;
        self.clock.set_rate(f64::from(rate));
        self.base.request_redraw();
    }

    /// Shows a decoded RGBA frame.
    pub fn set_frame(&mut self, frame: MediaFrame) {
        self.frame = Some(frame);
        self.base.request_redraw();
    }

    /// Removes the current frame, returning the player to its status-only view.
    pub fn clear_frame(&mut self) {
        if self.frame.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// Returns the current frame.
    pub fn frame(&self) -> Option<&MediaFrame> {
        self.frame.as_ref()
    }

    /// Replaces the playlist.
    pub fn set_playlist(&mut self, playlist: Vec<String>) {
        self.playlist = playlist;
        // A source that names a playlist entry keeps its position, so a host that sets the
        // list after the source does not lose the "where am I" answer.
        self.playlist_index = self
            .source
            .as_ref()
            .and_then(|source| self.playlist.iter().position(|item| item == source));
        self.base.request_redraw();
    }

    /// Returns the playlist.
    pub fn playlist(&self) -> &[String] {
        &self.playlist
    }

    /// The playlist entry currently loaded, if the source came from the playlist.
    pub fn playlist_index(&self) -> Option<usize> {
        self.playlist_index.filter(|index| *index < self.playlist.len())
    }

    /// Loads playlist entry `index`.
    pub fn select_track(&mut self, index: usize) -> bool {
        let Some(source) = self.playlist.get(index).cloned() else {
            return false;
        };
        self.playlist_index = Some(index);
        self.set_source(source, 0);
        true
    }

    /// Loads the next playlist entry.
    pub fn next_track(&mut self) -> bool {
        if self.playlist.is_empty() {
            return false;
        }
        let next = match self.playlist_index() {
            Some(index) => index + 1,
            None => 0,
        };
        self.select_track(next)
    }

    /// Loads the previous playlist entry.
    pub fn previous_track(&mut self) -> bool {
        let Some(index) = self.playlist_index() else {
            return false;
        };
        if index == 0 {
            return false;
        }
        self.select_track(index - 1)
    }

    /// Sets the chapter markers.
    pub fn set_chapters(&mut self, chapters: Vec<(u64, String)>) {
        self.chapters = chapters;
        self.base.request_redraw();
    }

    /// Returns the chapter markers.
    pub fn chapters(&self) -> &[(u64, String)] {
        &self.chapters
    }

    /// The progress rule's rectangle.
    fn progress_rect(&self) -> Rect {
        let rect = self.geometry();
        let height = dimensions::VIDEO_SEEK_BAR_HEIGHT;
        Rect::new(
            rect.x + MEDIA_PLAYER_PADDING,
            rect.y + rect.height as i32 - height as i32 - MEDIA_PLAYER_PADDING,
            rect.width.saturating_sub((MEDIA_PLAYER_PADDING * 2) as u32),
            height,
        )
    }

    /// The volume rail's rectangle, to the right of the transport buttons.
    fn volume_rect(&self) -> Rect {
        let bar = self.progress_rect();
        Rect::new(bar.x.saturating_add(42), bar.y.saturating_sub(12), 64.min(bar.width), 6)
    }

    /// The play/pause button, leading the transport row above the progress rule.
    fn play_rect(&self) -> Rect {
        let bar = self.progress_rect();
        let side = 16.min(bar.width);
        Rect::new(bar.x, bar.y.saturating_sub(18), side, side)
    }

    /// The mute button, beside the play button.
    fn mute_rect(&self) -> Rect {
        let bar = self.progress_rect();
        let side = 16.min(bar.width.saturating_sub(20));
        Rect::new(bar.x.saturating_add(20), bar.y.saturating_sub(18), side, side)
    }

    /// The chapter marker under `pos`, with its index.
    fn chapter_at(&self, pos: Point) -> Option<usize> {
        let bar = self.progress_rect();
        if self.duration_ms == 0 || bar.width == 0 {
            return None;
        }
        self.chapters.iter().position(|(position, _)| {
            let ratio = (*position as f64 / self.duration_ms as f64).clamp(0.0, 1.0) as f32;
            let x = bar.x + (bar.width as f32 * ratio).round() as i32;
            (pos.x - x).abs() <= 4
        })
    }

    /// Seeks to the ratio under `pos` on the progress rule.
    fn seek_from_point(&mut self, pos: Point) {
        let bar = self.progress_rect();
        if bar.width == 0 || self.duration_ms == 0 {
            return;
        }
        let ratio = ((pos.x - bar.x) as f32 / bar.width as f32).clamp(0.0, 1.0);
        self.seek_to((f64::from(ratio) * self.duration_ms as f64).round() as u64);
    }

    /// Sets the volume from a position on the volume rail.
    fn volume_from_point(&mut self, pos: Point) {
        let rail = self.volume_rect();
        if rail.width == 0 {
            return;
        }
        let ratio = ((pos.x - rail.x) as f32 / rail.width as f32).clamp(0.0, 1.0);
        self.set_volume((ratio * 100.0).round() as u8);
        self.set_muted(false);
    }

    /// Advances playback by `delta_secs`, and reports whether it is still running.
    ///
    /// # Why this exists (BLUE24 §1: a mechanism with no consumer)
    ///
    /// This control had everything a player needs **except the advancing**: a
    /// `position_ms`, a [`Self::progress_ratio`], a `position_changed` signal, a
    /// `play`/`pause`, and a progress rule in `draw` that reads the ratio. What it did
    /// not have was anything that *moved* `position_ms` while `playing` was true — so a
    /// started player painted a bar frozen at zero for ever, and the progress rule was
    /// dead code that happened to be reachable. This is the exact shape BLUE24 §0A.1
    /// measurement 1 recorded one layer up ("the tick has a caller, the caller has
    /// none"); here the position has a *reader* and no writer.
    ///
    /// # Why the delta is seconds and not milliseconds
    ///
    /// [`VideoPlayer`](crate::widget::media_widgets::video_player::VideoPlayer) — the
    /// sibling control in this family — already states playback time in seconds, because
    /// that is the unit a media clock is expressed in. Matching it means the two players
    /// cannot disagree about what a "tick" means, and the trait method converts once, at
    /// the boundary where the frame clock's unit (ms) is known. The alternative — an
    /// inherent millisecond tick and a seconds-based trait method — would put a
    /// conversion in the one place both players share, which is how the two drift.
    ///
    /// # Why it stops at the end rather than wrapping
    ///
    /// Reaching the end clears the playing state and emits `position_changed` with the final
    /// position, matching [`VideoPlayer::tick`]'s behaviour and the single playback-ended
    /// fact a host can act on. Wrapping would need a repeat mode, which this control does
    /// not have — inventing one here would be a second mechanism for a question the
    /// control has not been asked.
    ///
    /// # The arithmetic is the clock's, not this control's
    ///
    /// The advance used to be computed here — `position_ms + advance`, clamped against
    /// `duration_ms` — which is the same calculation [`MediaClock::tick`] performs on the same
    /// timeline this control now shares. Keeping one copy means the end rule, the rate and the clamp
    /// cannot come out different in the two places a caller can observe the position.
    pub fn tick(&mut self, delta_secs: f64) -> bool {
        if !self.is_playing_internal() || !delta_secs.is_finite() || delta_secs <= 0.0 {
            return false;
        }
        let before = self.clock.position();
        let after = self.clock.tick((delta_secs * 1000.0).round().max(0.0) as u32);
        if after != before {
            self.position_changed.emit(self.position_ms());
            self.base.request_redraw();
        }
        if self.clock.state() == crate::core::PlaybackState::Ended {
            // The end of the media: say so through the one signal that carries "playback state
            // changed". Returning `true` keeps the frame that observed the stop flowing, so the
            // pause is painted in the same frame it happened.
            self.playback_changed.emit(false);
            // A distinct end-of-media fact, so a host that wants to advance its playlist does
            // not have to infer "ended" from `playing == false` (a user pause has the same
            // value). Emitted on the same frame the clock reports `Ended`, once.
            self.media_ended.emit();
            return true;
        }
        true
    }

    fn progress_ratio(&self) -> f32 {
        if self.duration_ms == 0 {
            return 0.0;
        }
        (self.position_ms() as f32 / self.duration_ms as f32).clamp(0.0, 1.0)
    }
}

impl Widget for MediaPlayer {
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
        crate::core::Size::new(320, 240)
    }

    /// The frame clock's entry point; see [`MediaPlayer::tick`] for the unit and the stop rule.
    fn tick(&mut self, delta_ms: u32) -> bool {
        MediaPlayer::tick(self, f64::from(delta_ms) / 1000.0)
    }

    /// A playing player is the one thing about it that changes without an event.
    ///
    /// This is what makes [`crate::drive_frame`] keep asking for frames: a paused player
    /// answers `false` and costs a still frame nothing (BLUE24 §1 criterion 2).
    fn is_animating(&self) -> bool {
        self.is_playing_internal()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "playback_changed" => {
                Some(EventSignalRef::mapped("playback_changed", &self.playback_changed, |v| {
                    CapabilityValue::Bool(*v)
                }))
            }
            "position_changed" => {
                Some(EventSignalRef::mapped("position_changed", &self.position_changed, |v| {
                    CapabilityValue::UInt(*v)
                }))
            }
            "volume_changed" => {
                Some(EventSignalRef::mapped("volume_changed", &self.volume_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "source_changed" => {
                Some(EventSignalRef::mapped("source_changed", &self.source_changed, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "fullscreen_requested" => Some(EventSignalRef::mapped(
                "fullscreen_requested",
                &self.fullscreen_requested,
                |v| crate::widget::capability::CapabilityValue::Bool(*v),
            )),
            "media_ended" => Some(EventSignalRef::unit("media_ended", &self.media_ended)),
            "chapter_selected" => {
                Some(EventSignalRef::mapped("chapter_selected", &self.chapter_selected, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `MediaPlayer`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` / `access_write_other.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `MediaPlayer` reports
/// `WidgetKind::WebEngineView`, shared with `WebEngineView`; dispatching on the
/// concrete type here is what keeps the two contracts separate.
impl WidgetProperties for MediaPlayer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "source" => match self.source() {
                Some(source) => Ok(CapabilityValue::String(source.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "playing" => Ok(CapabilityValue::Bool(self.is_playing())),
            "duration_ms" => Ok(CapabilityValue::UInt(self.duration_ms())),
            "position_ms" => Ok(CapabilityValue::UInt(self.position_ms())),
            "volume" => Ok(CapabilityValue::UInt(self.volume() as u64)),
            "muted" => Ok(CapabilityValue::Bool(self.muted())),
            "fullscreen" => Ok(CapabilityValue::Bool(self.fullscreen())),
            "playlist_index" => match self.playlist_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "track_count" => Ok(CapabilityValue::UInt(self.playlist.len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "source" => match value {
                CapabilityValue::Null => {
                    self.clear_source();
                    Ok(())
                }
                other => {
                    let source = expect_string(other)?;
                    let duration = self.duration_ms();
                    self.set_source(source, duration);
                    Ok(())
                }
            },
            "playing" => {
                if expect_bool(value)? {
                    let _ = self.play();
                } else {
                    self.pause();
                }
                Ok(())
            }
            "duration_ms" => Err(CapabilityAccessError::ReadOnlyProperty),
            "position_ms" => {
                self.seek_to(expect_usize(value)? as u64);
                Ok(())
            }
            "volume" => {
                self.set_volume(expect_u32(value)? as u8);
                Ok(())
            }
            "muted" => {
                self.set_muted(expect_bool(value)?);
                Ok(())
            }
            "fullscreen" => {
                self.set_fullscreen(expect_bool(value)?);
                Ok(())
            }
            "playlist_index" => match value {
                CapabilityValue::UInt(index) => {
                    let index =
                        usize::try_from(index).map_err(|_| CapabilityAccessError::OutOfRange)?;
                    if self.select_track(index) {
                        Ok(())
                    } else {
                        Err(CapabilityAccessError::OutOfRange)
                    }
                }
                CapabilityValue::Null => {
                    self.playlist_index = None;
                    Ok(())
                }
                _ => Err(CapabilityAccessError::TypeMismatch),
            },
            "track_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "source",
            "playing",
            "duration_ms",
            "position_ms",
            "volume",
            "muted",
            "fullscreen",
            "playlist_index",
            "track_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `media_player` publishes.
    ///
    /// `clear_source`, `play` and `pause` are payload-free transport actions that
    /// flip state the control already owns, so a bare invocation performs them.
    /// `play` reports whether it could start: with no source loaded it returns
    /// `false`, and reporting success for playback that did not begin is exactly the
    /// silent-success failure this contract exists to prevent, so that case is
    /// [`CapabilityAccessError::OutOfRange`] — the same "argument missing" answer the
    /// index-addressed commands elsewhere in the crate give. `seek_to` needs the
    /// target position and is answered through the property route for the same reason.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_source" => {
                self.clear_source();
                Ok(())
            }
            "play" => {
                if self.play() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "pause" => {
                self.pause();
                Ok(())
            }
            "seek_to" | "set_source" | "set_volume" => Err(CapabilityAccessError::OutOfRange),
            "next_track" => {
                if self.next_track() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            "previous_track" => {
                if self.previous_track() {
                    Ok(())
                } else {
                    Err(CapabilityAccessError::OutOfRange)
                }
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for MediaPlayer {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::KeyPress { key, modifiers: _ } => match *key {
                32 => {
                    let _ = self.toggle_playback();
                }
                37 => self.seek_by(-5000),
                39 => self.seek_by(5000),
                38 => self.set_volume(self.volume.saturating_add(5).min(100)),
                40 => self.set_volume(self.volume.saturating_sub(5)),
                77 | 109 => self.toggle_mute(),
                70 | 102 => self.toggle_fullscreen(),
                // Brackets step the rate by a quarter, the convention a media player uses.
                91 | 219 => self.set_playback_rate(self.rate - 0.25),
                93 | 221 => self.set_playback_rate(self.rate + 0.25),
                // Unknown key; ignore
                _ => {}
            },
            Event::MousePress { pos, button: 1, .. } => {
                if self.play_rect().contains_point(*pos) {
                    let _ = self.toggle_playback();
                    return;
                }
                if self.mute_rect().contains_point(*pos) {
                    self.toggle_mute();
                    return;
                }
                if let Some(index) = self.chapter_at(*pos) {
                    if let Some((position, _)) = self.chapters.get(index) {
                        let position = *position;
                        self.seek_to(position);
                        self.chapter_selected.emit(index);
                    }
                    return;
                }
                let volume_rail = self.volume_rect();
                if volume_rail.contains_point(*pos) {
                    self.adjusting_volume = true;
                    self.volume_from_point(*pos);
                    return;
                }
                let bar_rect = self.progress_rect();
                if bar_rect.contains_point(*pos) {
                    self.scrubbing = true;
                    self.seek_from_point(*pos);
                }
            }
            Event::MouseMove { pos } => {
                if self.scrubbing {
                    self.seek_from_point(*pos);
                } else if self.adjusting_volume {
                    self.volume_from_point(*pos);
                }
            }
            Event::MouseRelease { .. } => {
                self.scrubbing = false;
                self.adjusting_volume = false;
            }
            Event::MouseLeave { .. } => {
                self.scrubbing = false;
                self.adjusting_volume = false;
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for MediaPlayer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // # Why this control reads the theme at all, given the `video-surface` exemption
        //
        // The exemption in `tools/control_color_exemptions.txt` covers a **decoded frame** — the
        // picture a player shows. This control paints no picture: it paints a title line, a status
        // line and a transport rule, which is **chrome**. So the exemption does not apply to the six
        // literals that were here, and they are why `audit_appearance.py` counted this file among the
        // Draw files that read no style at all.
        //
        // Precedence mirrors every other panel in the crate: an explicit style, then the active
        // theme's roles, then the literal as the last resort for a build with no theme.
        let themed = {
            let manager = crate::style::theme_manager();
            manager.current_theme().map(|active| {
                (
                    active.colors.surface_container,
                    active.colors.outline_variant,
                    active.colors.foreground,
                    active.colors.secondary,
                    active.colors.primary,
                )
            })
        };
        let (surface, divider, ink, muted, accent) = themed.unwrap_or((
            Color::rgb(24, 28, 36),
            Color::rgb(72, 84, 102),
            Color::rgb(232, 237, 245),
            Color::rgb(190, 202, 220),
            Color::rgb(107, 171, 248),
        ));
        let style = self.base.style();
        let surface = style.background_color.unwrap_or(surface);
        let divider = style.border_color.unwrap_or(divider);
        let ink = style.text_color.unwrap_or(ink);

        context.face_with_gradient(
            rect,
            surface,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect(rect, divider);

        // ── Picture ──
        //
        // Drawn over the status chrome, letterboxed inside the room between the two text lines
        // and the transport rule. A player with no frame keeps the status-only view it has
        // always had.
        if let Some(frame) = self.frame.as_ref() {
            let top = rect.y + MEDIA_PLAYER_PADDING + 36;
            let bottom = self.progress_rect().y - 6;
            if bottom > top {
                let area = Rect::new(
                    rect.x + MEDIA_PLAYER_PADDING,
                    top,
                    rect.width.saturating_sub((MEDIA_PLAYER_PADDING * 2) as u32),
                    (bottom - top) as u32,
                );
                let scale = (area.width as f32 / frame.width() as f32)
                    .min(area.height as f32 / frame.height() as f32);
                let width = (frame.width() as f32 * scale).round().max(1.0) as u32;
                let height = (frame.height() as f32 * scale).round().max(1.0) as u32;
                let x = area.x + (area.width as i32 - width as i32) / 2;
                let y = area.y + (area.height as i32 - height as i32) / 2;
                context.draw_image(x, y, width, height, frame.pixels());
            }
        }

        let title = self
            .source
            .as_deref()
            .map(|src| src.rsplit('/').next().unwrap_or(src))
            .unwrap_or("No media");
        let state = if self.is_playing_internal() { "Playing" } else { "Paused" };
        let fs = if self.fullscreen { "Fullscreen" } else { "Window" };
        // Rendered in place: the old `vol` string plus the join allocated two strings
        // every frame; one buffer takes the three fields directly.

        // Both lines are fitted to the control's width. The title comes from a file name,
        // which has no length limit, and the status line is three joined words; neither was
        // bounded, so a long file name simply kept going past the player's right edge. Both
        // share the one padding constant with the transport rule below them.
        // The title and status lines take the **effective font** — the resolved theme/caller font —
        // so the theme body font and the user's text scale reach them (D09-STYLE-01).
        let font = effective_font(style);
        context.draw_text_fitted_cow(
            Rect::new(
                rect.x + MEDIA_PLAYER_PADDING,
                rect.y + MEDIA_PLAYER_PADDING / 2,
                rect.width.saturating_sub((MEDIA_PLAYER_PADDING * 2) as u32),
                16,
            ),
            title,
            font,
            ink,
            HorizontalAlignment::Left,
        );
        let mut status = String::new();
        let _ = write!(status, "{state} | ");
        if self.muted {
            status.push_str("Muted");
        } else {
            let _ = write!(status, "Vol {}", self.volume);
        }
        let _ = write!(status, " | {fs}");
        context.draw_text_fitted_cow(
            Rect::new(
                rect.x + MEDIA_PLAYER_PADDING,
                rect.y + MEDIA_PLAYER_PADDING / 2 + 18,
                rect.width.saturating_sub((MEDIA_PLAYER_PADDING * 2) as u32),
                16,
            ),
            status.as_str(),
            font,
            muted,
            HorizontalAlignment::Left,
        );

        // The transport rule is pinned to the surface's bottom edge by the player's own
        // thickness rather than the literal `- 18`, which described one font size's layout:
        // the rule's offset from the bottom is its height plus the same 10 px margin the
        // labels above use, so the three stay in step when the surface changes size.
        let bar_height = dimensions::VIDEO_SEEK_BAR_HEIGHT;
        let bar_rect = Rect::new(
            rect.x + MEDIA_PLAYER_PADDING,
            rect.y + rect.height as i32 - bar_height as i32 - MEDIA_PLAYER_PADDING,
            rect.width.saturating_sub((MEDIA_PLAYER_PADDING * 2) as u32),
            bar_height,
        );
        // The track is a *derivative* of the surface rather than a fourth independent colour: it must
        // read as a recess in whatever the panel is, and stay there when the palette moves.
        context.fill_rect(bar_rect, surface.blend(&ink, 0.14));
        let fill_w = ((bar_rect.width as f32) * self.progress_ratio()) as u32;
        if fill_w > 0 {
            // The progress fill is a **value indicator**, so it is the accent — exactly as a
            // slider's fill and a progress bar's are — rather than a fixed blue.
            context.fill_rect(Rect::new(bar_rect.x, bar_rect.y, fill_w, bar_rect.height), accent);
        }
        // Chapter markers: a thin notch at each chapter's position, so the structure of the
        // media is readable from the transport rule without opening a menu.
        if self.duration_ms > 0 {
            for (position, _) in &self.chapters {
                let ratio = (*position as f64 / self.duration_ms as f64).clamp(0.0, 1.0) as f32;
                let x = bar_rect.x + (bar_rect.width as f32 * ratio).round() as i32;
                context.fill_rect(Rect::new(x, bar_rect.y, 1, bar_rect.height), ink);
            }
        }

        // The volume rail sits just above the transport rule, showing the level the pointer or
        // the arrow keys have chosen.
        let volume_rail = self.volume_rect();
        context.fill_rect(volume_rail, surface.blend(&ink, 0.14));
        let volume_w = if self.muted {
            0
        } else {
            ((volume_rail.width as f32) * (f32::from(self.volume) / 100.0)).round() as u32
        };
        if volume_w > 0 {
            context.fill_rect(
                Rect::new(volume_rail.x, volume_rail.y, volume_w, volume_rail.height),
                accent,
            );
        }
        // The rate is shown only when it is not real time, so the ordinary view stays uncluttered.
        if (self.rate - 1.0).abs() > f32::EPSILON {
            let rate_text = format!("{:.2}x", self.rate);
            context.draw_text(
                Point::new(volume_rail.x + volume_rail.width as i32 + 6, volume_rail.y - 2),
                &rate_text,
                font,
                muted,
                HorizontalAlignment::Left,
            );
        }

        // ── Transport buttons ──
        //
        // Dedicated hit regions rather than "a press anywhere that is not a rail toggles
        // playback". The old rule made every stray click on the panel start or stop the media.
        let play = self.play_rect();
        if play.width > 0 {
            let icon = if self.is_playing_internal() { IconName::Pause } else { IconName::Play };
            crate::widget::draw_icon_centered(
                context,
                play,
                play.width.min(play.height),
                ink,
                icon,
            );
        }
        let mute = self.mute_rect();
        if mute.width > 0 {
            let icon = if self.muted { IconName::VolumeOff } else { IconName::VolumeUp };
            crate::widget::draw_icon_centered(
                context,
                mute,
                mute.width.min(mute.height),
                muted,
                icon,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// The panel's surface follows the appearance instead of a fixed dark blue.
    ///
    /// # The defect this pins
    ///
    /// Every colour in `draw` was a literal — a `rgb(24,28,36)` panel, a `rgb(72,84,102)` frame,
    /// two inks, a `rgb(62,73,90)` track and a `rgb(107,171,248)` progress fill. The control paints
    /// **chrome** (a title line, a status line and a transport rule), not a decoded frame, so the
    /// `video-surface` exemption does not cover any of them: a light build got a dark panel that no
    /// theme could reach, and the progress fill was a fixed blue rather than the accent.
    ///
    /// The sample point is on the panel **below the two label lines and left of the transport rule's
    /// start**, so the pixel it reads is the surface and nothing else.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_panel_surface_follows_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();

        let surface_pixel = |appearance| -> (u8, u8, u8) {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let mut player = MediaPlayer::new(Rect::new(0, 0, 320, 200));
            crate::theme::apply_theme_to_widget(&mut player);
            let svg =
                crate::widget::svg::render_widget_to_svg(&mut player, Rect::new(0, 0, 320, 200));
            // The panel is the **first coloured** `<rect>`: the renderer emits its own backdrop first
            // (the same colour in both appearances), then the control paints its surface over it.
            // Taking the *last* match was tried and rejected by measurement — it lands on the
            // transport track, which is a derived colour in both renders and so made the assertion
            // pass with the panel restored to the literal.
            let mut fills: Vec<(u8, u8, u8)> = Vec::new();
            for element in svg.split("/>") {
                let Some(open) = element.find("<rect ") else { continue };
                let element = &element[open + "<rect ".len()..];
                let Some(from) = element.find("fill=\"rgba(") else { continue };
                let from = from + "fill=\"rgba(".len();
                let Some(to) = element[from..].find(')') else { continue };
                let mut parts = element[from..from + to].split(',');
                let r: u8 = parts.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                let g: u8 = parts.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                let b: u8 = parts.next().and_then(|v| v.trim().parse().ok()).unwrap_or(0);
                fills.push((r, g, b));
            }
            assert!(fills.len() > 1, "the player paints a surface over the backdrop");
            fills[1]
        };

        let dark = surface_pixel(crate::theme::AppearanceMode::Dark);
        let light = surface_pixel(crate::theme::AppearanceMode::Light);
        assert_ne!(dark, light, "the panel surface must follow the appearance; both were {dark:?}");
        assert_ne!(dark, (24, 28, 36), "the surface must not be the fixed dark blue");
    }

    #[test]
    fn source_set_resets_position_and_state() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 320, 200));
        player.set_source("/tmp/video.mp4", 120_000);

        assert_eq!(player.source(), Some("/tmp/video.mp4"));
        assert_eq!(player.duration_ms(), 120_000);
        assert_eq!(player.position_ms(), 0);
        assert!(!player.is_playing());
    }

    #[test]
    fn narrow_progress_bar_does_not_divide_by_zero() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 10, 80));
        player.set_source("/tmp/video.mp4", 120_000);
        player.handle_event(&Event::MousePress { pos: Point::new(5, 62), button: 1, modifiers: 0 });
        assert_eq!(player.position_ms(), 0);
    }

    #[test]
    fn playback_and_seek_update_state() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 320, 200));
        player.set_source("demo.mp3", 10_000);

        assert!(player.play());
        assert!(player.is_playing());

        player.seek_by(2500);
        assert_eq!(player.position_ms(), 2500);

        player.seek_by(-5000);
        assert_eq!(player.position_ms(), 0);

        player.pause();
        assert!(!player.is_playing());
    }

    /// A playing player advances with the frame clock, and a paused one does not.
    ///
    /// # The defect this pins
    ///
    /// Before `tick` existed, `play()` set `playing = true` and **nothing ever moved
    /// `position_ms`**: the control had a position field, a progress ratio and a progress
    /// rule in `draw`, all of them readable and none of them written. A started player
    /// painted a bar frozen at zero for ever. This asserts the writer exists.
    #[test]
    fn a_playing_player_advances_with_the_frame_clock() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 320, 200));
        player.set_source("demo.mp3", 10_000);

        // Paused: a frame costs nothing, which is what keeps a still window free (BLUE24 §1
        // criterion 2).
        assert!(!player.tick(0.016), "a paused player does not advance");
        assert_eq!(player.position_ms(), 0);

        assert!(player.play());
        assert!(player.tick(0.5), "a playing player reports that it is still running");
        assert_eq!(player.position_ms(), 500, "half a second of media time passed");
        assert!(player.is_animating(), "and it answers the frame loop's question");

        // Pausing stops the clock at the position it reached, rather than resetting it.
        player.pause();
        assert!(!player.tick(0.5));
        assert_eq!(player.position_ms(), 500);
        assert!(!player.is_animating());
    }

    /// Reaching the end stops playback and clamps at the duration.
    ///
    /// The alternative readings are both wrong in ways a user would notice: wrapping needs
    /// a repeat mode this control does not have, and overshooting the duration would make
    /// [`MediaPlayer::progress_ratio`] exceed `1.0` on the frame the media ends.
    #[test]
    fn playback_stops_at_the_end_and_clamps() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 320, 200));
        player.set_source("short.mp3", 1_000);
        assert!(player.play());

        // A single long frame overshoots; the position must clamp, not run past the end.
        assert!(player.tick(5.0), "the frame that observes the stop keeps flowing");
        assert_eq!(player.position_ms(), 1_000, "the position clamps at the duration");
        assert!(!player.is_playing(), "and playback has stopped");
        assert!((player.progress_ratio() - 1.0).abs() < 1e-6, "a finished player is full");
        assert!(!player.is_animating(), "so the frame loop can go back to sleep");

        // A degenerate frame (no media, or a non-finite delta) must not advance or panic.
        let mut idle = MediaPlayer::new(Rect::new(0, 0, 320, 200));
        assert!(!idle.tick(0.016), "a player with no source does not advance");
        idle.set_source("x.mp3", 5_000);
        assert!(idle.play());
        assert!(!idle.tick(f64::NAN), "a non-finite delta advances nothing");
        assert_eq!(idle.position_ms(), 0, "rather than poisoning the position");
    }

    #[test]
    fn signals_emit_on_state_changes() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 320, 200));
        player.set_source("demo.mp4", 20_000);

        let playback = Arc::new(Mutex::new(Vec::<bool>::new()));
        let playback_sink = playback.clone();
        player.playback_changed.connect(move |state| {
            if let Ok(mut guard) = playback_sink.lock() {
                guard.push(*state);
            }
        });

        let volume = Arc::new(Mutex::new(Vec::<u8>::new()));
        let volume_sink = volume.clone();
        player.volume_changed.connect(move |value| {
            if let Ok(mut guard) = volume_sink.lock() {
                guard.push(*value);
            }
        });

        let _ = player.play();
        player.set_volume(65);
        player.pause();

        let playback_events = playback.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(playback_events, vec![true, false]);

        let volume_events = volume.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(volume_events, vec![65]);
    }

    /// The playlist is navigable and the end of the media is its own signal.
    #[test]
    fn playlist_navigation_and_end_signal() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 320, 200));
        player.set_playlist(vec!["a.mp3".to_string(), "b.mp3".to_string()]);
        assert_eq!(player.playlist_index(), None);
        assert!(player.next_track());
        assert_eq!(player.source(), Some("a.mp3"));
        assert_eq!(player.playlist_index(), Some(0));
        assert!(player.next_track());
        assert_eq!(player.source(), Some("b.mp3"));
        assert!(!player.next_track(), "past the last entry");
        assert!(player.previous_track());
        assert_eq!(player.playlist_index(), Some(0));

        // The end-of-media signal is distinct from a pause: `playback_changed` carries both.
        let ended = player.media_ended.clone();
        let count = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let sink = std::sync::Arc::clone(&count);
        ended.connect(move || *sink.lock().unwrap() += 1);
        player.set_source("short.mp3", 1_000);
        assert!(player.play());
        assert!(player.tick(5.0));
        assert_eq!(*count.lock().unwrap(), 1);
        assert!(player.event_signal_dyn("media_ended").is_some());
    }

    /// Chapter markers are clickable and report their index.
    #[test]
    fn a_chapter_marker_seeks_and_reports_its_index() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 320, 200));
        player.set_source("movie.mp4", 100_000);
        player.set_chapters(vec![(50_000, "Middle".to_string())]);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = std::sync::Arc::clone(&seen);
        player.chapter_selected.connect(move |index| sink.lock().unwrap().push(*index));

        let bar = player.progress_rect();
        let x = bar.x + bar.width as i32 / 2;
        player.handle_event(&Event::mouse_press(x, bar.y + 1, 1));
        assert_eq!(*seen.lock().unwrap(), vec![0]);
        assert_eq!(player.position_ms(), 50_000);
    }

    /// The mute button has a hit region of its own; a blank panel press no longer toggles play.
    #[test]
    fn the_mute_button_toggles_and_blank_clicks_do_not_play() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 320, 200));
        player.set_source("clip.mp4", 10_000);
        let mute = player.mute_rect();
        player.handle_event(&Event::mouse_press(mute.x + 2, mute.y + 2, 1));
        assert!(player.muted());

        // A press on the empty panel (the centre of the title band) does nothing.
        assert!(!player.is_playing());
        player.handle_event(&Event::mouse_press(160, 8, 1));
        assert!(!player.is_playing(), "a stray click must not start playback");
    }

    #[test]
    fn new_creates_default_state() {
        let player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        assert_eq!(player.source(), None);
        assert!(!player.is_playing());
        assert_eq!(player.duration_ms(), 0);
        assert_eq!(player.position_ms(), 0);
        assert_eq!(player.volume(), 80);
        assert!(!player.muted());
        assert!(!player.fullscreen());
    }

    #[test]
    fn play_without_source_returns_false() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        assert!(!player.play());
        assert!(!player.is_playing());
    }

    #[test]
    fn toggle_playback_both_directions() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        player.set_source("clip.mp4", 30_000);

        // Toggle on
        assert!(player.toggle_playback());
        assert!(player.is_playing());

        // Toggle off
        assert!(player.toggle_playback());
        assert!(!player.is_playing());
    }

    #[test]
    fn clear_source_removes_source_and_resets_state() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        player.set_source("video.mp4", 60_000);
        player.seek_to(15_000);
        let _ = player.play();

        player.clear_source();
        assert_eq!(player.source(), None);
        assert_eq!(player.duration_ms(), 0);
        assert_eq!(player.position_ms(), 0);
        assert!(!player.is_playing());
    }

    #[test]
    fn volume_clamp_upper_bound() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        player.set_volume(200);
        assert_eq!(player.volume(), 100);
    }

    #[test]
    fn volume_guard_no_op() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        player.set_volume(80);
        // Second call with same value should not emit
        let emitted = Arc::new(Mutex::new(Vec::<u8>::new()));
        let sink = emitted.clone();
        player.volume_changed.connect(move |v| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(*v);
            }
        });
        player.set_volume(80);
        let got = emitted.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert_eq!(got.len(), 0);
    }

    #[test]
    fn mute_get_set_and_toggle() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        assert!(!player.muted());

        player.set_muted(true);
        assert!(player.muted());

        player.toggle_mute();
        assert!(!player.muted());

        // Guard: same value no-op
        player.set_muted(false);
        assert!(!player.muted());
    }

    #[test]
    fn seek_to_clamps_to_duration() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        player.set_source("track.mp3", 10_000);

        player.seek_to(20_000);
        assert_eq!(player.position_ms(), 10_000);
    }

    #[test]
    fn seek_by_handles_negative_overshoot() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        player.set_source("track.mp3", 10_000);
        player.seek_to(5_000);

        player.seek_by(-10_000);
        assert_eq!(player.position_ms(), 0);
    }

    #[test]
    fn fullscreen_guard_and_toggle() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        assert!(!player.fullscreen());

        player.set_fullscreen(true);
        assert!(player.fullscreen());

        // Guard: same value no-op
        player.set_fullscreen(true);
        assert!(player.fullscreen());

        player.toggle_fullscreen();
        assert!(!player.fullscreen());
    }

    #[test]
    fn clear_source_emits_playback_and_position_signals() {
        let mut player = MediaPlayer::new(Rect::new(0, 0, 800, 600));
        player.set_source("video.mp4", 30_000);

        let playback_events = Arc::new(Mutex::new(Vec::<bool>::new()));
        let ps = playback_events.clone();
        player.playback_changed.connect(move |s| {
            if let Ok(mut guard) = ps.lock() {
                guard.push(*s);
            }
        });
        let position_events = Arc::new(Mutex::new(Vec::<u64>::new()));
        let pos_s = position_events.clone();
        player.position_changed.connect(move |p| {
            if let Ok(mut guard) = pos_s.lock() {
                guard.push(*p);
            }
        });

        player.clear_source();

        // set_source emits (false, 0); clear_source also emits (false, 0)
        let pb = playback_events.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert!(pb.contains(&false));
        let pos = position_events.lock().ok().map(|g| g.clone()).unwrap_or_default();
        assert!(pos.contains(&0));
    }
}
