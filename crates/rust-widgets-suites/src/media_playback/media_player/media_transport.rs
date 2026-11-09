// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MediaTransportBar` — a reusable transport bar a host hangs under a media control.
//!
//! The media controls in `media_widgets` (`VideoPlayer`, `AnimatedImage`, `LottieWidget`,
//! `RiveWidget`, `CameraPreview`) each paint their own transport chrome, so a host that wants one
//! consistent bar cannot have it. This companion owns that bar: playback state, a draggable
//! progress track, volume, fullscreen and an auto-hide clock. It owns no media state machine —
//! every user intent is a request signal, and the host remains the authority on position and
//! duration (COMP-L01).
//!
//! Registration: canonical name `media_transport_bar`, kind `WidgetKind::VideoPlayer`,
//! registered by `capability::registration` with its own property table (RW-81).

use crate::media_playback::video_player::{MediaStatus, TransportRequest, VideoPlayer};
use rust_widgets::compat::{Arc, Mutex, String};
use rust_widgets::core::{Color, HorizontalAlignment, Point, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{ConnectionScope, EventSignalRef, GenericSignal, Signal1};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::role_font;
use rust_widgets::widget::{draw_icon_at, BaseWidget, Draw, IconName, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};

/// The kind of media control the bar is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaTargetKind {
    /// No media control bound yet.
    #[default]
    None,
    /// A [`crate::media_playback::video_player::VideoPlayer`].
    VideoPlayer,
    /// An [`rust_widgets::widget::media_widgets::AnimatedImage`].
    AnimatedImage,
    /// A [`crate::media_playback::lottie_widget::LottieWidget`].
    LottieWidget,
    /// A [`crate::media_playback::rive_widget::RiveWidget`].
    RiveWidget,
    /// A [`crate::media_playback::camera_preview::CameraPreview`].
    CameraPreview,
}

/// A transport bar for an arbitrary media control.
pub struct MediaTransportBar {
    base: BaseWidget,
    target_kind: MediaTargetKind,
    playing: bool,
    /// Playback position and duration in milliseconds.
    position_ms: u64,
    duration_ms: u64,
    volume: f32,
    muted: bool,
    /// The volume in force before the mute button zeroed it (MP-06).
    ///
    /// Muting emits `volume_changed(0.0)` and the player applies it, so `current_volume()` reads
    /// `0.0` when the same button is pressed to un-mute. Remembering the pre-mute value is what
    /// lets "unmute" actually restore something instead of re-sending `0.0`.
    pre_mute_volume: Option<f32>,
    auto_hide_ms: u64,
    idle_ms: u64,
    visible: bool,
    show_volume: bool,
    show_fullscreen: bool,
    show_timecode: bool,
    /// Whether the previous/next-track controls are shown (MW-001).
    show_next_previous: bool,
    accent: Option<Color>,
    /// Whether the progress track is being dragged.
    scrubbing: bool,
    /// The progress track from the last paint, so a drag maps to the painted span.
    track_rect: Option<Rect>,
    /// The last formatted timecode, keyed by the `(position, duration)` it was built from.
    timecode_cache: (u64, u64, String),
    /// The play/pause button from the last paint.
    play_rect: Option<Rect>,
    /// The volume button from the last paint.
    volume_rect: Option<Rect>,
    /// The fullscreen button from the last paint.
    fullscreen_rect: Option<Rect>,
    /// The previous-track button from the last paint (MW-001).
    previous_rect: Option<Rect>,
    /// The next-track button from the last paint (MW-001).
    next_rect: Option<Rect>,
    /// Emitted when the previous-track control is activated.
    pub previous_track_requested: GenericSignal,
    /// Emitted when the next-track control is activated.
    pub next_track_requested: GenericSignal,
    /// Emitted with the new desired playing state.
    pub play_toggled: Signal1<bool>,
    /// Emitted with the requested position as a `0.0..=1.0` fraction.
    pub seek_requested: Signal1<f32>,
    /// Emitted with the requested volume, `0.0..=1.0`.
    pub volume_changed: Signal1<f32>,
    /// Emitted when the fullscreen control is activated.
    pub fullscreen_requested: GenericSignal,
    /// The live binding to a player's state, when one is followed.
    binding: Option<TransportBinding>,
}

/// The state a followed player reports, shared with the signal callbacks.
#[derive(Debug, Clone, Copy)]
struct TransportMirror {
    playing: bool,
    position_ms: u64,
    duration_ms: u64,
    volume: f32,
    muted: bool,
    /// Set by a callback so the animation bus knows a frame is owed.
    dirty: bool,
}

/// A continuous subscription to a player's playback, clock, volume and mute signals.
struct TransportBinding {
    mirror: Arc<Mutex<TransportMirror>>,
    _scope: ConnectionScope,
    /// The connections that forward this bar's requests into an attached player (MW-001).
    ///
    /// Kept beside the mirror subscriptions rather than inside `_scope` so `attach_video_player`
    /// can replace the forwarding without disturbing the state mirror.
    requests: Option<ConnectionScope>,
}

impl MediaTransportBar {
    /// Creates a bar with the default visibility and auto-hide settings.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::VideoPlayer, geometry, "MediaTransportBar"),
            target_kind: MediaTargetKind::None,
            playing: false,
            position_ms: 0,
            duration_ms: 0,
            volume: 0.8,
            muted: false,
            pre_mute_volume: None,
            auto_hide_ms: 3000,
            idle_ms: 0,
            visible: true,
            show_volume: true,
            show_fullscreen: true,
            show_timecode: true,
            show_next_previous: false,
            accent: None,
            scrubbing: false,
            track_rect: None,
            timecode_cache: (0, 0, timecode(0, 0)),
            play_rect: None,
            volume_rect: None,
            fullscreen_rect: None,
            previous_rect: None,
            next_rect: None,
            previous_track_requested: GenericSignal::new(),
            next_track_requested: GenericSignal::new(),
            play_toggled: Signal1::new(),
            seek_requested: Signal1::new(),
            volume_changed: Signal1::new(),
            fullscreen_requested: GenericSignal::new(),
            binding: None,
        }
    }

    /// Binds a video player, adopting its current playback state.
    pub fn bind_video_player(
        &mut self,
        player: &mut crate::media_playback::video_player::VideoPlayer,
    ) {
        // An explicit snapshot supersedes a live binding; otherwise the followed mirror would
        // keep winning every read below.
        self.binding = None;
        self.target_kind = MediaTargetKind::VideoPlayer;
        self.playing = player.is_playing();
        self.position_ms = (player.current_time() * 1000.0) as u64;
        self.duration_ms = (player.duration() * 1000.0) as u64;
        self.volume = player.volume();
        self.muted = player.is_muted();
        self.base.request_redraw();
    }

    /// Follows `player` continuously instead of adopting a one-time snapshot.
    ///
    /// Every read of the bar's state goes through the shared mirror once this is called, so the
    /// transport reflects the player's clock, playback state, volume and mute as they change. The
    /// previous binding (and its subscriptions) is replaced atomically (MW-016).
    pub fn follow_video_player(&mut self, player: &VideoPlayer) {
        let mirror = Arc::new(Mutex::new(TransportMirror {
            playing: player.is_playing(),
            position_ms: (player.current_time().max(0.0) * 1000.0) as u64,
            duration_ms: (player.duration().max(0.0) * 1000.0) as u64,
            volume: player.volume(),
            muted: player.is_muted(),
            dirty: false,
        }));
        let scope = ConnectionScope::new();

        let sink = Arc::clone(&mirror);
        player.time_updated.connect_ref_scoped(&scope, move |time| {
            let mut state = rust_widgets::compat::lock(&sink);
            let position = (time.max(0.0) * 1000.0) as u64;
            if state.position_ms != position {
                state.position_ms = position;
                state.dirty = true;
            }
        });
        let sink = Arc::clone(&mirror);
        player.media_status_changed.connect_ref_scoped(&scope, move |status| {
            let mut state = rust_widgets::compat::lock(&sink);
            let playing = *status == MediaStatus::Playing;
            if state.playing != playing {
                state.playing = playing;
                state.dirty = true;
            }
        });
        let sink = Arc::clone(&mirror);
        player.volume_changed.connect_ref_scoped(&scope, move |volume| {
            let mut state = rust_widgets::compat::lock(&sink);
            if (state.volume - *volume).abs() >= f32::EPSILON {
                state.volume = *volume;
                state.dirty = true;
            }
        });
        let sink = Arc::clone(&mirror);
        player.mute_changed.connect_ref_scoped(&scope, move |muted| {
            let mut state = rust_widgets::compat::lock(&sink);
            if state.muted != *muted {
                state.muted = *muted;
                state.dirty = true;
            }
        });
        let sink = Arc::clone(&mirror);
        player.duration_changed.connect_ref_scoped(&scope, move |duration| {
            let mut state = rust_widgets::compat::lock(&sink);
            let duration = (duration.max(0.0) * 1000.0) as u64;
            if state.duration_ms != duration {
                state.duration_ms = duration;
                state.dirty = true;
            }
        });

        self.binding = Some(TransportBinding { mirror, _scope: scope, requests: None });
        self.target_kind = MediaTargetKind::VideoPlayer;
        self.base.request_redraw();
    }

    /// Attaches this bar to `player`: follows its state **and** forwards the bar's own requests
    /// back to it (MW-001).
    ///
    /// [`Self::follow_video_player`] only mirrors the player, which left the round trip to the
    /// host: every intent was a signal and nothing on this side ever reached the player. This
    /// method closes that loop with the same shared-queue mechanism the chapter list uses
    /// (`VideoPlayer::accept_transport_requests`): the bar writes a [`TransportRequest`], the
    /// player drains it on its next paint or interaction. The signals still fire, so a host that
    /// wants to observe or veto an intent can connect to them as before.
    ///
    /// Calling this again replaces the previous attachment, subscriptions included.
    pub fn attach_video_player(&mut self, player: &mut VideoPlayer) {
        self.follow_video_player(player);

        let queue: Arc<Mutex<rust_widgets::compat::Vec<TransportRequest>>> =
            Arc::new(Mutex::new(rust_widgets::compat::Vec::new()));
        let redraw = player.accept_transport_requests(Arc::clone(&queue));
        let scope = ConnectionScope::new();

        let sink = Arc::clone(&queue);
        let wake = redraw.clone();
        self.play_toggled.connect_ref_scoped(&scope, move |playing| {
            let request = if *playing { TransportRequest::Play } else { TransportRequest::Pause };
            rust_widgets::compat::lock(&sink).push(request);
            wake.emit();
        });

        let sink = Arc::clone(&queue);
        let wake = redraw.clone();
        self.seek_requested.connect_ref_scoped(&scope, move |fraction| {
            rust_widgets::compat::lock(&sink).push(TransportRequest::Seek(*fraction));
            wake.emit();
        });

        let sink = Arc::clone(&queue);
        let wake = redraw.clone();
        self.volume_changed.connect_ref_scoped(&scope, move |volume| {
            rust_widgets::compat::lock(&sink).push(TransportRequest::Volume(*volume));
            wake.emit();
        });

        let sink = Arc::clone(&queue);
        let wake = redraw.clone();
        self.previous_track_requested.connect_scoped(&scope, move || {
            rust_widgets::compat::lock(&sink).push(TransportRequest::PreviousTrack);
            wake.emit();
        });

        let sink = Arc::clone(&queue);
        let wake = redraw.clone();
        self.next_track_requested.connect_scoped(&scope, move || {
            rust_widgets::compat::lock(&sink).push(TransportRequest::NextTrack);
            wake.emit();
        });

        if let Some(binding) = self.binding.as_mut() {
            binding.requests = Some(scope);
        }
        self.target_kind = MediaTargetKind::VideoPlayer;
        self.base.request_redraw();
    }

    /// Whether the bar is following a player's live state.
    pub fn is_following_player(&self) -> bool {
        self.binding.is_some()
    }

    /// Stops following and keeps the mirrored values as the local snapshot.
    pub fn stop_following_player(&mut self) {
        if let Some(binding) = self.binding.take() {
            let state = *rust_widgets::compat::lock(&binding.mirror);
            self.playing = state.playing;
            self.position_ms = state.position_ms;
            self.duration_ms = state.duration_ms;
            self.volume = state.volume;
            self.muted = state.muted;
            self.base.request_redraw();
        }
    }

    /// The playing state currently shown.
    fn current_playing(&self) -> bool {
        match &self.binding {
            Some(binding) => rust_widgets::compat::lock(&binding.mirror).playing,
            None => self.playing,
        }
    }

    /// The position and duration currently shown.
    fn current_position(&self) -> (u64, u64) {
        match &self.binding {
            Some(binding) => {
                let state = rust_widgets::compat::lock(&binding.mirror);
                (state.position_ms, state.duration_ms)
            }
            None => (self.position_ms, self.duration_ms),
        }
    }

    /// The volume currently shown.
    fn current_volume(&self) -> f32 {
        match &self.binding {
            Some(binding) => rust_widgets::compat::lock(&binding.mirror).volume,
            None => self.volume,
        }
    }

    /// The mute state currently shown.
    fn current_muted(&self) -> bool {
        match &self.binding {
            Some(binding) => rust_widgets::compat::lock(&binding.mirror).muted,
            None => self.muted,
        }
    }

    /// Writes a value into the shared mirror when one is bound.
    fn write_mirror(&mut self, update: impl FnOnce(&mut TransportMirror)) {
        if let Some(binding) = &self.binding {
            update(&mut rust_widgets::compat::lock(&binding.mirror));
        }
    }

    /// Binds an animated image, adopting its current playback state.
    pub fn bind_animated_image(
        &mut self,
        image: &mut rust_widgets::widget::media_widgets::AnimatedImage,
    ) {
        self.binding = None;
        self.target_kind = MediaTargetKind::AnimatedImage;
        self.playing = image.is_playing();
        self.base.request_redraw();
    }

    /// Binds a Lottie animation, adopting its current playback state.
    pub fn bind_lottie(&mut self, lottie: &mut crate::media_playback::lottie_widget::LottieWidget) {
        self.binding = None;
        self.target_kind = MediaTargetKind::LottieWidget;
        self.playing = lottie.is_playing();
        self.base.request_redraw();
    }

    /// Binds a Rive animation, adopting its current playback state.
    pub fn bind_rive(&mut self, rive: &mut crate::media_playback::rive_widget::RiveWidget) {
        self.binding = None;
        self.target_kind = MediaTargetKind::RiveWidget;
        self.playing = rive.is_playing();
        self.base.request_redraw();
    }

    /// The kind of control the bar is bound to.
    pub fn target_kind(&self) -> MediaTargetKind {
        self.target_kind
    }

    /// How long the bar stays visible after the last pointer activity; `0` keeps it up.
    pub fn auto_hide_ms(&self) -> u64 {
        self.auto_hide_ms
    }

    /// Sets the auto-hide delay.
    pub fn set_auto_hide_ms(&mut self, ms: u64) {
        if self.auto_hide_ms == ms {
            return;
        }
        self.auto_hide_ms = ms;
        self.idle_ms = 0;
        if ms == 0 {
            self.visible = true;
        }
        self.base.request_redraw();
    }

    /// Resets the auto-hide countdown and shows the bar.
    pub fn notify_pointer_activity(&mut self) {
        self.idle_ms = 0;
        if !self.visible {
            self.visible = true;
            self.base.request_redraw();
        }
    }

    /// Updates the playback state shown by the play/pause control.
    pub fn set_playing(&mut self, playing: bool) {
        if self.current_playing() == playing {
            return;
        }
        self.playing = playing;
        self.write_mirror(|state| state.playing = playing);
        self.base.request_redraw();
    }

    /// Emits the previous-track request (RW-106).
    ///
    /// This is the signal the previous button emits, exposed as an operation so a design
    /// document can request it without simulating a press. The bar changes no state: the player
    /// it is attached to decides what "previous" means.
    pub fn request_previous_track(&mut self) {
        self.previous_track_requested.emit();
    }

    /// Emits the next-track request (RW-106), the signal the next button emits.
    pub fn request_next_track(&mut self) {
        self.next_track_requested.emit();
    }

    /// Emits the fullscreen request (RW-106), the signal the fullscreen button emits.
    pub fn request_fullscreen(&mut self) {
        self.fullscreen_requested.emit();
    }

    /// Whether the bar believes playback is active.
    pub fn is_playing(&self) -> bool {
        self.current_playing()
    }

    /// Sets the position and duration shown by the track.
    pub fn set_position(&mut self, position_ms: u64, duration_ms: u64) {
        if self.current_position() == (position_ms, duration_ms) {
            return;
        }
        self.position_ms = position_ms;
        self.duration_ms = duration_ms;
        self.write_mirror(|state| {
            state.position_ms = position_ms;
            state.duration_ms = duration_ms;
        });
        self.base.request_redraw();
    }

    /// The shown position and duration in milliseconds.
    pub fn position(&self) -> (u64, u64) {
        self.current_position()
    }

    /// Sets the volume, clamped to `0.0..=1.0`.
    pub fn set_volume(&mut self, volume: f32) {
        let volume = if volume.is_finite() { volume.clamp(0.0, 1.0) } else { return };
        if (self.current_volume() - volume).abs() < f32::EPSILON {
            return;
        }
        self.volume = volume;
        self.write_mirror(|state| state.volume = volume);
        self.base.request_redraw();
    }

    /// The shown volume.
    pub fn volume(&self) -> f32 {
        self.current_volume()
    }

    /// Hides or shows the volume control.
    pub fn set_show_volume(&mut self, show: bool) {
        if self.show_volume == show {
            return;
        }
        self.show_volume = show;
        self.base.request_redraw();
    }

    /// Shows or hides the previous/next-track controls (MW-001).
    ///
    /// The bar cannot hold the player (a binding is a one-way mirror), so the controls emit
    /// [`Self::previous_track_requested`] / [`Self::next_track_requested`]; the host connects
    /// them to `VideoPlayer::previous_track` / `VideoPlayer::next_track`.
    pub fn set_show_next_previous(&mut self, show: bool) {
        if self.show_next_previous == show {
            return;
        }
        self.show_next_previous = show;
        self.base.request_redraw();
    }

    /// Whether the previous/next-track controls are shown.
    pub fn show_next_previous(&self) -> bool {
        self.show_next_previous
    }

    /// Hides or shows the fullscreen control.
    pub fn set_show_fullscreen(&mut self, show: bool) {
        if self.show_fullscreen == show {
            return;
        }
        self.show_fullscreen = show;
        self.base.request_redraw();
    }

    /// Hides or shows the timecode.
    pub fn set_show_timecode(&mut self, show: bool) {
        if self.show_timecode == show {
            return;
        }
        self.show_timecode = show;
        self.base.request_redraw();
    }

    /// Sets an explicit accent colour for the progress fill; `None` follows the theme.
    pub fn set_accent(&mut self, accent: Option<Color>) {
        if self.accent == accent {
            return;
        }
        self.accent = accent;
        self.base.request_redraw();
    }

    /// The bar's painted strip: the control's own rectangle.
    fn strip(&self) -> Rect {
        self.geometry()
    }

    /// The progress fraction, `0.0..=1.0`.
    fn progress(&self) -> f32 {
        let (position_ms, duration_ms) = self.current_position();
        if duration_ms == 0 {
            0.0
        } else {
            (position_ms as f32 / duration_ms as f32).clamp(0.0, 1.0)
        }
    }

    /// Maps an x coordinate on the track to a position fraction.
    fn fraction_at(&self, x: i32) -> f32 {
        let Some(track) = self.track_rect else {
            return self.progress();
        };
        if track.width == 0 {
            return 0.0;
        }
        ((x - track.x) as f32 / track.width as f32).clamp(0.0, 1.0)
    }

    /// Requests a seek and optimistically moves the shown position.
    fn request_seek(&mut self, fraction: f32) {
        let fraction = fraction.clamp(0.0, 1.0);
        let (_, duration_ms) = self.current_position();
        let position_ms = (duration_ms as f32 * fraction) as u64;
        self.position_ms = position_ms;
        self.write_mirror(|state| state.position_ms = position_ms);
        self.seek_requested.emit(fraction);
        self.base.request_redraw();
    }
}

impl Widget for MediaTransportBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("media_transport_bar")
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }
    fn size_hint(&self) -> Size {
        Size::new(320, 40)
    }
    /// Drives the auto-hide clock and reports a followed player's changes.
    ///
    /// `auto_hide_ms` is a published property, so it needs an execution site: this is the only
    /// place a wall-clock delta reaches the bar, and [`Self::notify_pointer_activity`] is the
    /// reset half. Once the countdown reaches the delay the bar hides and owes one repaint;
    /// further ticks change nothing.
    ///
    /// A countdown that is still running makes the control **animating**: the frame bus only
    /// ticks controls that answer `is_animating` with `true`, so without that override this
    /// method was unreachable on a mounted bar and `auto_hide_ms` promised a behaviour that
    /// never ran.
    fn is_animating(&self) -> bool {
        (self.auto_hide_ms > 0 && self.visible && self.idle_ms < self.auto_hide_ms)
            || self
                .binding
                .as_ref()
                .is_some_and(|binding| rust_widgets::compat::lock(&binding.mirror).dirty)
    }
    fn tick(&mut self, delta_ms: u32) -> bool {
        let mut redrew = false;
        // A drag owns the bar: holding the track is pointer activity, and hiding the bar under the
        // pointer froze the handle (the retired track rect made every later drag request the
        // current progress).
        if self.scrubbing {
            self.idle_ms = 0;
        } else {
            self.idle_ms = self.idle_ms.saturating_add(u64::from(delta_ms));
        }
        if self.auto_hide_ms > 0
            && self.idle_ms >= self.auto_hide_ms
            && self.visible
            && !self.scrubbing
        {
            self.visible = false;
            self.base.request_redraw();
            redrew = true;
        }
        let dirty = self
            .binding
            .as_ref()
            .is_some_and(|binding| rust_widgets::compat::lock(&binding.mirror).dirty);
        if dirty {
            self.write_mirror(|state| state.dirty = false);
            self.base.request_redraw();
            redrew = true;
        }
        redrew
    }
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "fullscreen_requested" => {
                Some(EventSignalRef::unit("fullscreen_requested", &self.fullscreen_requested))
            }
            "next_track_requested" => {
                Some(EventSignalRef::unit("next_track_requested", &self.next_track_requested))
            }
            "previous_track_requested" => Some(EventSignalRef::unit(
                "previous_track_requested",
                &self.previous_track_requested,
            )),
            "play_toggled" => {
                Some(EventSignalRef::mapped("play_toggled", &self.play_toggled, |value| {
                    CapabilityValue::Bool(*value)
                }))
            }
            "seek_requested" => {
                Some(EventSignalRef::mapped("seek_requested", &self.seek_requested, |value| {
                    CapabilityValue::Float(*value as f64)
                }))
            }
            "volume_changed" => {
                Some(EventSignalRef::mapped("volume_changed", &self.volume_changed, |value| {
                    CapabilityValue::Float(*value as f64)
                }))
            }
            _ => None,
        }
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `MediaTransportBar`'s property contract (RW-81).
///
/// The bar is registered under `WidgetKind::VideoPlayer` — the control it drives — with its own
/// canonical name `media_transport_bar`. Position and duration are read-only: the host owns the
/// media clock, and a property write would race it.
impl WidgetProperties for MediaTransportBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "playing" => Ok(CapabilityValue::Bool(self.is_playing())),
            "position_ms" => Ok(CapabilityValue::UInt(self.position().0)),
            "duration_ms" => Ok(CapabilityValue::UInt(self.position().1)),
            "volume" => Ok(CapabilityValue::Float(f64::from(self.volume()))),
            "auto_hide_ms" => Ok(CapabilityValue::UInt(self.auto_hide_ms())),
            "show_volume" => Ok(CapabilityValue::Bool(self.show_volume)),
            "show_timecode" => Ok(CapabilityValue::Bool(self.show_timecode)),
            "show_fullscreen" => Ok(CapabilityValue::Bool(self.show_fullscreen)),
            "show_next_previous" => Ok(CapabilityValue::Bool(self.show_next_previous())),
            "accent" => Ok(match self.accent {
                Some(color) => CapabilityValue::Color(color),
                None => CapabilityValue::Null,
            }),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "playing" => {
                let playing = match value {
                    CapabilityValue::Bool(playing) => playing,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_playing(playing);
                Ok(())
            }
            "volume" => {
                let volume = rust_widgets::widget::capability::coercion::expect_f32(value)?;
                self.set_volume(volume);
                Ok(())
            }
            "auto_hide_ms" => {
                let ms = match value {
                    CapabilityValue::UInt(ms) => ms,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_auto_hide_ms(ms);
                Ok(())
            }
            "show_volume" => {
                let show = match value {
                    CapabilityValue::Bool(show) => show,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_show_volume(show);
                Ok(())
            }
            "show_timecode" => {
                let show = match value {
                    CapabilityValue::Bool(show) => show,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_show_timecode(show);
                Ok(())
            }
            "show_fullscreen" => {
                let show = match value {
                    CapabilityValue::Bool(show) => show,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_show_fullscreen(show);
                Ok(())
            }
            "show_next_previous" => {
                let show = match value {
                    CapabilityValue::Bool(show) => show,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_show_next_previous(show);
                Ok(())
            }
            "accent" => {
                let accent = match value {
                    CapabilityValue::Color(color) => Some(color),
                    CapabilityValue::Null => None,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_accent(accent);
                Ok(())
            }
            // The clock belongs to the host; `set_position` exists for a Rust caller that owns it.
            "position_ms" | "duration_ms" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "playing",
            "position_ms",
            "duration_ms",
            "volume",
            "auto_hide_ms",
            "show_volume",
            "show_timecode",
            "show_fullscreen",
            "show_next_previous",
            "accent",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs the commands `media_transport_bar` publishes (RW-106).
    ///
    /// The three payload-free names emit exactly the signals the bar's own buttons emit, so a
    /// wire and a click cannot describe different requests. `set_playing` / `set_volume` carry
    /// their values through the property route and are answered by the convention.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "previous_track" => {
                self.request_previous_track();
                Ok(())
            }
            "next_track" => {
                self.request_next_track();
                Ok(())
            }
            "toggle_fullscreen" => {
                self.request_fullscreen();
                Ok(())
            }
            _ => self.default_command(name),
        }
    }
}

impl Draw for MediaTransportBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let strip = self.strip();
        if strip.width == 0 || strip.height == 0 {
            return;
        }
        let style = self.base.style();
        // MP-02: this bar is chrome a host hangs *under* a media control, not the decoded picture
        // the `video-surface` exemption covers. Resolve palette roles first (surface / outline /
        // foreground / primary), let an explicit style colour win, and derive the icon ink from
        // the strip it sits on — the same three-level precedence `MediaPlayer::draw` uses.
        let themed = rust_widgets::style::palette_shared().map(|colors| {
            (colors.surface_container, colors.outline, colors.foreground, colors.primary)
        });
        let (surface, outline, foreground, accent_default) = themed.unwrap_or((
            // The old literals, kept as the no-theme fallback: the translucent dark strip, its
            // grey track, the light timecode ink and the progress fill.
            Color::rgba(0, 0, 0, 160),
            Color::rgba(100, 100, 100, 200),
            Color::rgba(200, 200, 200, 230),
            Color::rgba(80, 150, 230, 240),
        ));
        let strip_fill = style.background_color.unwrap_or(surface);
        let track_fill = style.border_color.unwrap_or(outline);
        let muted_ink = style.text_color.unwrap_or(foreground);
        let accent = self.accent.unwrap_or(accent_default);
        // The icons are drawn on the strip, so their ink is the strip's own contrast colour; on
        // the default dark strip this is the old `Color::WHITE`.
        let icon_ink = strip_fill.contrast_color();
        let font = &role_font(style, 12.0);

        context.fill_rect(strip, strip_fill);
        if !self.visible {
            // A hidden bar paints no controls, so the hit regions the last visible paint
            // recorded must go with them: a press on the strip would otherwise still toggle
            // play or start a drag on a control nobody can see.
            self.play_rect = None;
            self.volume_rect = None;
            self.fullscreen_rect = None;
            self.previous_rect = None;
            self.next_rect = None;
            self.track_rect = None;
            return;
        }

        let button = strip.height.saturating_sub(8).min(24);
        let button_y = strip.y + (strip.height as i32 - button as i32) / 2;
        let mut left = strip.x + 4;
        if self.show_next_previous {
            let previous = Rect::new(left, button_y, button, button);
            left = previous.right() + 6;
            draw_icon_at(context, previous, icon_ink, IconName::ArrowLeft);
            self.previous_rect = Some(previous);
        } else {
            self.previous_rect = None;
        }
        let play = Rect::new(left, button_y, button, button);
        left = play.right() + 6;
        self.play_rect = Some(play);
        draw_icon_at(
            context,
            play,
            icon_ink,
            if self.current_playing() { IconName::Pause } else { IconName::Play },
        );
        if self.show_next_previous {
            let next = Rect::new(left, button_y, button, button);
            draw_icon_at(context, next, icon_ink, IconName::ArrowRight);
            self.next_rect = Some(next);
        } else {
            self.next_rect = None;
        }

        // Right-hand controls are laid out from the trailing edge first.
        let mut right = strip.right() - 4;
        let fullscreen = if self.show_fullscreen {
            let rect = Rect::new(
                right - button as i32,
                strip.y + (strip.height as i32 - button as i32) / 2,
                button,
                button,
            );
            right = rect.x - 6;
            draw_icon_at(context, rect, icon_ink, IconName::Fullscreen);
            self.fullscreen_rect = Some(rect);
            Some(rect)
        } else {
            self.fullscreen_rect = None;
            None
        };
        let _ = fullscreen;
        if self.show_volume {
            let rect = Rect::new(
                right - button as i32,
                strip.y + (strip.height as i32 - button as i32) / 2,
                button,
                button,
            );
            right = rect.x - 6;
            let icon = if self.current_muted() || self.current_volume() < 0.01 {
                IconName::VolumeOff
            } else {
                IconName::VolumeUp
            };
            draw_icon_at(context, rect, icon_ink, icon);
            self.volume_rect = Some(rect);
        } else {
            self.volume_rect = None;
        }

        // The timecode occupies the slot left of the right-hand group.
        let time_text = if self.show_timecode {
            let (position_ms, duration_ms) = self.current_position();
            // The clock text is constant between seconds, so it is rebuilt only when that second
            // pair changes.
            let (position_s, duration_s) = (position_ms / 1000, duration_ms / 1000);
            if self.timecode_cache.0 != position_s || self.timecode_cache.1 != duration_s {
                self.timecode_cache = (position_s, duration_s, timecode(position_ms, duration_ms));
            }
            let text = self.timecode_cache.2.as_str();
            let metrics = context.measure_text(text, font);
            let x = right - metrics.width as i32 - 6;
            let y = strip.y + (strip.height as i32 - metrics.height as i32) / 2;
            context.draw_text(Point::new(x, y), text, font, muted_ink, HorizontalAlignment::Left);
            right = x - 6;
            true
        } else {
            false
        };
        let _ = time_text;

        // What remains is the track. It starts after the last leading control, not after play:
        // with next/previous enabled, `play.right() + 8` overlapped the next button by 22 px, so
        // the track was painted across its glyph while a press there hit "next".
        let track_x =
            self.next_rect.or(self.play_rect).map_or(play.right(), |rect| rect.right()) + 8;
        let track_width = (right - track_x).max(0) as u32;
        let track_y = strip.y + strip.height as i32 / 2 - 2;
        let track = Rect::new(track_x, track_y, track_width, 4);
        self.track_rect = Some(track);
        context.fill_rounded_rect(track, 2, track_fill);
        let filled = (track.width as f32 * self.progress()) as u32;
        if filled > 0 {
            context.fill_rounded_rect(Rect::new(track.x, track.y, filled, track.height), 2, accent);
        }
    }
}

impl EventHandler for MediaTransportBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                if self.strip().contains_point(*pos) {
                    self.notify_pointer_activity();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if !self.strip().contains_point(*pos) {
                    return;
                }
                let was_visible = self.visible;
                self.notify_pointer_activity();
                // A hidden bar paints no controls, and its hit rectangles are only retired inside
                // `draw`. A press between the hide and the repaint must reveal the bar, not act on
                // a region it is not painting yet.
                if !was_visible {
                    return;
                }
                if self.previous_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    self.previous_track_requested.emit();
                    return;
                }
                if self.next_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    self.next_track_requested.emit();
                    return;
                }
                if self.play_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    let playing = !self.current_playing();
                    self.set_playing(playing);
                    self.play_toggled.emit(playing);
                    self.base.request_redraw();
                    return;
                }
                if self.fullscreen_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    self.fullscreen_requested.emit();
                    return;
                }
                if self.volume_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    let muted = !self.current_muted();
                    self.muted = muted;
                    self.write_mirror(|state| state.muted = muted);
                    // MP-06: the mute request is `volume_changed(0.0)`, which the player applies,
                    // so `current_volume()` reads `0.0` by the time the same button is pressed
                    // again. Remember the value being muted and restore it on un-mute; only a bar
                    // with no remembered value falls back to the current volume.
                    let volume = if muted {
                        let current = self.current_volume();
                        self.pre_mute_volume = Some(current);
                        0.0
                    } else {
                        self.pre_mute_volume.take().unwrap_or_else(|| self.current_volume())
                    };
                    self.volume_changed.emit(volume);
                    self.base.request_redraw();
                    return;
                }
                if self.track_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    self.scrubbing = true;
                    self.request_seek(self.fraction_at(pos.x));
                }
            }
            Event::Drag { pos, .. } if self.scrubbing => {
                self.request_seek(self.fraction_at(pos.x));
            }
            Event::MouseRelease { .. } if self.scrubbing => {
                self.scrubbing = false;
            }
            // A release can land outside the control, and focus can move mid-drag;
            // both leave the scrub latch set unless it is cleared here.
            Event::MouseLeave { .. } | Event::FocusLost if self.scrubbing => {
                self.scrubbing = false;
            }
            _ => {}
        }
    }
}

/// Formats a media clock as `mm:ss / mm:ss`.
fn timecode(position_ms: u64, duration_ms: u64) -> String {
    use core::fmt::Write as _;
    let mut text = String::new();
    let position_s = position_ms / 1000;
    let duration_s = duration_ms / 1000;
    let _ = write!(
        text,
        "{:02}:{:02}/{:02}:{:02}",
        position_s / 60,
        position_s % 60,
        duration_s / 60,
        duration_s % 60
    );
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_widgets::render::PaintBackend;

    /// RW-81: the registration-backed property contract round-trips its writable half and
    /// honestly refuses the host-owned clock.
    #[test]
    fn property_contract_round_trips() {
        use rust_widgets::widget::capability::WidgetProperties;
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        bar.set("playing", CapabilityValue::Bool(true)).expect("playing is writable");
        assert!(bar.is_playing());
        bar.set("volume", CapabilityValue::Float(0.25)).expect("volume is writable");
        assert!((bar.volume() - 0.25).abs() < f32::EPSILON);
        assert_eq!(bar.get("playing"), Ok(CapabilityValue::Bool(true)));
        assert_eq!(
            bar.set("position_ms", CapabilityValue::UInt(1)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn a_press_toggles_the_playing_request() {
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        bar.play_toggled.connect(move |value: std::sync::Arc<bool>| {
            sink.lock().unwrap().push(*value);
        });
        // Paint once so the button rectangles are known.
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(Size::new(320, 40), 1.0);
        backend.begin_frame(Color::BLACK);
        bar.draw(&mut RenderContext::new(&mut backend));
        backend.end_frame();
        let play = bar.play_rect.expect("the play button was painted");
        bar.handle_event(&Event::MousePress {
            pos: Point::new(play.center().x, play.center().y),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(*seen.lock().unwrap(), vec![true]);
    }

    #[test]
    fn dragging_the_track_requests_a_fraction() {
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        bar.set_position(0, 10_000);
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        bar.seek_requested.connect(move |value: std::sync::Arc<f32>| {
            sink.lock().unwrap().push(*value);
        });
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(Size::new(320, 40), 1.0);
        backend.begin_frame(Color::BLACK);
        bar.draw(&mut RenderContext::new(&mut backend));
        backend.end_frame();
        let track = bar.track_rect.expect("the track was painted");
        bar.handle_event(&Event::MousePress {
            pos: Point::new(track.x + track.width as i32 / 2, track.center().y),
            button: 1,
            modifiers: 0,
        });
        let last = *seen.lock().unwrap().last().expect("a seek was requested");
        assert!((last - 0.5).abs() < 0.05, "mid-track is half the timeline, got {last}");
    }

    /// MP-01: `auto_hide_ms` is published, so the tick must actually run the countdown.
    ///
    /// Before this the delta was discarded (`tick(&mut self, _delta_ms)`) and `idle_ms` had no
    /// increment site, so a bar set to hide after 3000 ms stayed up forever.
    #[test]
    fn the_transport_bar_hides_itself_once_the_auto_hide_delay_elapses() {
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        bar.set_auto_hide_ms(100);
        assert!(bar.visible, "a fresh bar is up");

        assert!(!bar.tick(40), "less than the delay changes nothing, so no frame is owed");
        assert!(bar.visible);
        assert!(bar.tick(60), "reaching the delay owes one repaint");
        assert!(!bar.visible, "the bar hides once the delay has elapsed");
        assert!(!bar.tick(1000), "a hidden bar has nothing left to change");

        bar.notify_pointer_activity();
        assert!(bar.visible, "pointer activity brings the bar back");
        assert_eq!(bar.idle_ms, 0, "and resets the countdown");

        bar.set_auto_hide_ms(0);
        assert!(!bar.tick(10_000), "`0` keeps the bar up");
        assert!(bar.visible);
    }

    /// MP-06: un-muting restores the volume that muting zeroed.
    #[test]
    fn the_mute_button_restores_the_volume_it_muted() {
        use std::sync::{Arc, Mutex};

        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        bar.set_volume(0.8);
        let seen = Arc::new(Mutex::new(Vec::<f32>::new()));
        let sink = seen.clone();
        bar.volume_changed.connect(move |value: Arc<f32>| {
            sink.lock().unwrap().push(*value);
        });

        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(Size::new(320, 40), 1.0);
        backend.begin_frame(Color::BLACK);
        bar.draw(&mut RenderContext::new(&mut backend));
        backend.end_frame();
        let volume = bar.volume_rect.expect("the volume button was painted");
        let press = || Event::MousePress {
            pos: Point::new(volume.center().x, volume.center().y),
            button: 1,
            modifiers: 0,
        };

        // Muting asks the player for 0.0 and remembers what it replaced.
        bar.handle_event(&press());
        assert_eq!(*seen.lock().unwrap(), vec![0.0]);
        assert_eq!(bar.pre_mute_volume, Some(0.8));

        // The player applied the request, which is what made `current_volume()` read 0.0.
        bar.set_volume(0.0);
        assert!((bar.volume() - 0.0).abs() < f32::EPSILON);

        // Un-muting restores the remembered value instead of re-sending 0.0.
        bar.handle_event(&press());
        assert_eq!(*seen.lock().unwrap(), vec![0.0, 0.8]);
        assert_eq!(bar.pre_mute_volume, None, "the remembered value is consumed once");
    }

    /// A followed player drives the bar through its signals rather than a one-time snapshot
    /// (MW-016).
    #[test]
    fn following_a_player_updates_the_bar_and_owes_a_frame() {
        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.load("clip.mp4");
        player.set_duration(60.0);
        player.set_volume(0.4);
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        bar.follow_video_player(&player);
        assert!(bar.is_following_player());
        assert!(!bar.is_playing());
        assert!((bar.volume() - 0.4).abs() < f32::EPSILON);

        player.play();
        assert!(bar.is_playing(), "the play signal reaches the bar");
        player.seek(30.0);
        assert_eq!(bar.position(), (30_000, 60_000));
        assert!(bar.tick(16), "a followed change owes a frame");
        assert!(!bar.tick(16), "the change was consumed");

        player.set_volume(0.2);
        assert!((bar.volume() - 0.2).abs() < f32::EPSILON);
        bar.stop_following_player();
        assert!(!bar.is_following_player());
        assert_eq!(bar.position(), (30_000, 60_000));
    }

    /// MW-001: an attached bar closes the loop through the player's request queue.
    ///
    /// `follow_video_player` only mirrors the player, so before this the round trip was the host's
    /// to build. This asserts the attached form end to end: each control's signal queues a
    /// [`TransportRequest`], and the player applies it through the same public method a host would
    /// call.
    #[test]
    fn an_attached_bar_drives_the_player_through_its_queue() {
        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_playlist(vec!["a.mp4".to_string(), "b.mp4".to_string(), "c.mp4".to_string()]);
        player.load("a.mp4");
        player.set_duration(60.0);
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        bar.attach_video_player(&mut player);
        assert!(bar.is_following_player(), "the bar mirrors the player it forwards to");

        bar.play_toggled.emit(true);
        assert_eq!(player.apply_pending_transport_requests(), 1);
        assert!(player.is_playing(), "the play request reaches the player");

        assert_eq!(player.playlist_index(), Some(0), "the loaded source is the first entry");

        bar.next_track_requested.emit();
        assert_eq!(player.apply_pending_transport_requests(), 1);
        assert_eq!(player.playlist_index(), Some(1));

        bar.next_track_requested.emit();
        assert_eq!(player.apply_pending_transport_requests(), 1);
        assert_eq!(player.playlist_index(), Some(2));

        bar.previous_track_requested.emit();
        assert_eq!(player.apply_pending_transport_requests(), 1);
        assert_eq!(player.playlist_index(), Some(1), "previous returns to the entry it left");

        bar.seek_requested.emit(0.5);
        assert_eq!(player.apply_pending_transport_requests(), 1);
        assert_eq!(player.current_time(), 30.0, "the fraction is resolved against the duration");

        bar.volume_changed.emit(0.25);
        assert_eq!(player.apply_pending_transport_requests(), 1);
        assert!((player.volume() - 0.25).abs() < f32::EPSILON);

        assert_eq!(player.apply_pending_transport_requests(), 0, "the queue is drained");
    }

    /// RW-106: the three request operations emit exactly the signals the buttons emit.
    ///
    /// Publishing a command that quietly did nothing would be the "reported success for
    /// something that did not happen" this suite's contracts forbid, so the operations are
    /// asserted through their signals rather than by their `Ok(())`.
    #[test]
    fn the_request_operations_emit_the_button_signals() {
        use rust_widgets::signal::ConnectionScope;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        let scope = ConnectionScope::new();

        let previous = Arc::new(AtomicUsize::new(0));
        let next = Arc::new(AtomicUsize::new(0));
        let fullscreen = Arc::new(AtomicUsize::new(0));
        {
            let counter = Arc::clone(&previous);
            bar.previous_track_requested.connect_scoped(&scope, move || {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let counter = Arc::clone(&next);
            bar.next_track_requested.connect_scoped(&scope, move || {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }
        {
            let counter = Arc::clone(&fullscreen);
            bar.fullscreen_requested.connect_scoped(&scope, move || {
                counter.fetch_add(1, Ordering::SeqCst);
            });
        }

        bar.request_previous_track();
        bar.request_next_track();
        bar.request_fullscreen();
        assert_eq!(previous.load(Ordering::SeqCst), 1);
        assert_eq!(next.load(Ordering::SeqCst), 1);
        assert_eq!(fullscreen.load(Ordering::SeqCst), 1);
    }

    /// A hidden bar paints no controls, so their hit regions must be retired with them.
    #[test]
    fn a_hidden_bar_has_no_hit_regions() {
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(Size::new(320, 40), 1.0);
        backend.begin_frame(Color::BLACK);
        bar.draw(&mut RenderContext::new(&mut backend));
        backend.end_frame();
        let play = bar.play_rect.expect("the play button was painted");
        let toggles = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sink = toggles.clone();
        bar.play_toggled.connect(move |_value: std::sync::Arc<bool>| {
            sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        bar.set_auto_hide_ms(10);
        assert!(bar.tick(10), "the countdown reaches the delay");
        assert!(!bar.visible, "the bar hid itself");
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(Size::new(320, 40), 1.0);
        backend.begin_frame(Color::BLACK);
        bar.draw(&mut RenderContext::new(&mut backend));
        backend.end_frame();
        assert!(bar.play_rect.is_none(), "a hidden bar paints no play button");

        bar.handle_event(&Event::MousePress {
            pos: Point::new(play.center().x, play.center().y),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(
            toggles.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "no hidden control may fire"
        );
    }

    /// A drag owns the bar: holding the track must not let the countdown hide it.
    #[test]
    fn the_bar_stays_up_while_the_track_is_dragged() {
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(Size::new(320, 40), 1.0);
        backend.begin_frame(Color::BLACK);
        bar.draw(&mut RenderContext::new(&mut backend));
        backend.end_frame();
        let track = bar.track_rect.expect("the track was painted");

        bar.set_auto_hide_ms(100);
        bar.handle_event(&Event::MousePress {
            pos: Point::new(track.x + 2, track.center().y),
            button: 1,
            modifiers: 0,
        });
        assert!(bar.scrubbing, "the press grabs the track");

        assert!(!bar.tick(5_000), "a drag owns the bar, so no frame is owed");
        assert!(bar.visible, "the bar must not vanish under the pointer");
    }

    /// A hidden bar reveals itself on a press instead of acting on retired hit regions.
    #[test]
    fn a_hidden_bar_does_not_fire_before_it_repaints() {
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(Size::new(320, 40), 1.0);
        backend.begin_frame(Color::BLACK);
        bar.draw(&mut RenderContext::new(&mut backend));
        backend.end_frame();
        let play = bar.play_rect.expect("the play button was painted");
        let toggles = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let sink = toggles.clone();
        bar.play_toggled.connect(move |_value: std::sync::Arc<bool>| {
            sink.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        bar.set_auto_hide_ms(10);
        assert!(bar.tick(10));
        assert!(!bar.visible, "the bar hid itself");
        // No paint since hiding: the recorded rect still describes a hidden control.
        bar.handle_event(&Event::MousePress {
            pos: Point::new(play.center().x, play.center().y),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(
            toggles.load(std::sync::atomic::Ordering::SeqCst),
            0,
            "a press must reveal, not act on a region the bar is not painting"
        );
        assert!(bar.visible, "the press brings the bar back");
    }

    /// The progress track starts after the last leading button, not under it.
    ///
    /// With next/previous enabled the track began at `play.right() + 8`, overlapping the next
    /// button; its fill was painted across the glyph while a press there reported "next".
    #[test]
    fn the_track_does_not_overlap_the_step_buttons() {
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        bar.set_show_next_previous(true);
        let mut backend = rust_widgets::render::SoftwarePaintBackend::new(Size::new(320, 40), 1.0);
        backend.begin_frame(Color::BLACK);
        bar.draw(&mut RenderContext::new(&mut backend));
        backend.end_frame();

        let next = bar.next_rect.expect("the next button is painted");
        let track = bar.track_rect.expect("the track is painted");
        assert!(
            track.x >= next.right(),
            "the track must start after the next button, got {track:?} vs {next:?}"
        );
    }
}
