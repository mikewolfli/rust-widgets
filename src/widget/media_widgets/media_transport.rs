// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MediaTransportBar` — a reusable transport bar a host hangs under a media control.
//!
//! The media controls in this folder (`VideoPlayer`, `AnimatedImage`, `LottieWidget`,
//! `RiveWidget`, `CameraPreview`) each paint their own transport chrome, so a host that wants one
//! consistent bar cannot have it. This companion owns that bar: playback state, a draggable
//! progress track, volume, fullscreen and an auto-hide clock. It owns no media state machine —
//! every user intent is a request signal, and the host remains the authority on position and
//! duration (COMP-L01).
//!
//! Registration surface (out of scope for this file, recorded for the capability table):
//! canonical name `media_transport_bar`, kind [`WidgetKind::VideoPlayer`].

use crate::compat::{Arc, Mutex, String};
use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal, Signal1};
use crate::widget::media_widgets::video_player::{MediaStatus, TransportRequest, VideoPlayer};
use crate::widget::metrics::role_font;
use crate::widget::{draw_icon_at, BaseWidget, Draw, IconName, Widget, WidgetKind};

/// The kind of media control the bar is attached to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MediaTargetKind {
    /// No media control bound yet.
    #[default]
    None,
    /// A [`crate::widget::media_widgets::VideoPlayer`].
    VideoPlayer,
    /// An [`crate::widget::media_widgets::AnimatedImage`].
    AnimatedImage,
    /// A [`crate::widget::media_widgets::LottieWidget`].
    LottieWidget,
    /// A [`crate::widget::media_widgets::RiveWidget`].
    RiveWidget,
    /// A [`crate::widget::media_widgets::CameraPreview`].
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
    pub fn bind_video_player(&mut self, player: &mut crate::widget::media_widgets::VideoPlayer) {
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
            let mut state = crate::compat::lock(&sink);
            let position = (time.max(0.0) * 1000.0) as u64;
            if state.position_ms != position {
                state.position_ms = position;
                state.dirty = true;
            }
        });
        let sink = Arc::clone(&mirror);
        player.media_status_changed.connect_ref_scoped(&scope, move |status| {
            let mut state = crate::compat::lock(&sink);
            let playing = *status == MediaStatus::Playing;
            if state.playing != playing {
                state.playing = playing;
                state.dirty = true;
            }
        });
        let sink = Arc::clone(&mirror);
        player.volume_changed.connect_ref_scoped(&scope, move |volume| {
            let mut state = crate::compat::lock(&sink);
            if (state.volume - *volume).abs() >= f32::EPSILON {
                state.volume = *volume;
                state.dirty = true;
            }
        });
        let sink = Arc::clone(&mirror);
        player.mute_changed.connect_ref_scoped(&scope, move |muted| {
            let mut state = crate::compat::lock(&sink);
            if state.muted != *muted {
                state.muted = *muted;
                state.dirty = true;
            }
        });
        let sink = Arc::clone(&mirror);
        player.duration_changed.connect_ref_scoped(&scope, move |duration| {
            let mut state = crate::compat::lock(&sink);
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

        let queue: Arc<Mutex<crate::compat::Vec<TransportRequest>>> =
            Arc::new(Mutex::new(crate::compat::Vec::new()));
        let redraw = player.accept_transport_requests(Arc::clone(&queue));
        let scope = ConnectionScope::new();

        let sink = Arc::clone(&queue);
        let wake = redraw.clone();
        self.play_toggled.connect_ref_scoped(&scope, move |playing| {
            let request = if *playing { TransportRequest::Play } else { TransportRequest::Pause };
            crate::compat::lock(&sink).push(request);
            wake.emit();
        });

        let sink = Arc::clone(&queue);
        let wake = redraw.clone();
        self.seek_requested.connect_ref_scoped(&scope, move |fraction| {
            crate::compat::lock(&sink).push(TransportRequest::Seek(*fraction));
            wake.emit();
        });

        let sink = Arc::clone(&queue);
        let wake = redraw.clone();
        self.volume_changed.connect_ref_scoped(&scope, move |volume| {
            crate::compat::lock(&sink).push(TransportRequest::Volume(*volume));
            wake.emit();
        });

        let sink = Arc::clone(&queue);
        let wake = redraw.clone();
        self.previous_track_requested.connect_scoped(&scope, move || {
            crate::compat::lock(&sink).push(TransportRequest::PreviousTrack);
            wake.emit();
        });

        let sink = Arc::clone(&queue);
        let wake = redraw.clone();
        self.next_track_requested.connect_scoped(&scope, move || {
            crate::compat::lock(&sink).push(TransportRequest::NextTrack);
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
            let state = *crate::compat::lock(&binding.mirror);
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
            Some(binding) => crate::compat::lock(&binding.mirror).playing,
            None => self.playing,
        }
    }

    /// The position and duration currently shown.
    fn current_position(&self) -> (u64, u64) {
        match &self.binding {
            Some(binding) => {
                let state = crate::compat::lock(&binding.mirror);
                (state.position_ms, state.duration_ms)
            }
            None => (self.position_ms, self.duration_ms),
        }
    }

    /// The volume currently shown.
    fn current_volume(&self) -> f32 {
        match &self.binding {
            Some(binding) => crate::compat::lock(&binding.mirror).volume,
            None => self.volume,
        }
    }

    /// The mute state currently shown.
    fn current_muted(&self) -> bool {
        match &self.binding {
            Some(binding) => crate::compat::lock(&binding.mirror).muted,
            None => self.muted,
        }
    }

    /// Writes a value into the shared mirror when one is bound.
    fn write_mirror(&mut self, update: impl FnOnce(&mut TransportMirror)) {
        if let Some(binding) = &self.binding {
            update(&mut crate::compat::lock(&binding.mirror));
        }
    }

    /// Binds an animated image, adopting its current playback state.
    pub fn bind_animated_image(&mut self, image: &mut crate::widget::media_widgets::AnimatedImage) {
        self.binding = None;
        self.target_kind = MediaTargetKind::AnimatedImage;
        self.playing = image.is_playing();
        self.base.request_redraw();
    }

    /// Binds a Lottie animation, adopting its current playback state.
    pub fn bind_lottie(&mut self, lottie: &mut crate::widget::media_widgets::LottieWidget) {
        self.binding = None;
        self.target_kind = MediaTargetKind::LottieWidget;
        self.playing = lottie.is_playing();
        self.base.request_redraw();
    }

    /// Binds a Rive animation, adopting its current playback state.
    pub fn bind_rive(&mut self, rive: &mut crate::widget::media_widgets::RiveWidget) {
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
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn size_hint(&self) -> Size {
        Size::new(320, 40)
    }
    /// Reports a frame when a followed player has changed the shown state.
    fn tick(&mut self, _delta_ms: u32) -> bool {
        let dirty =
            self.binding.as_ref().is_some_and(|binding| crate::compat::lock(&binding.mirror).dirty);
        if !dirty {
            return false;
        }
        self.write_mirror(|state| state.dirty = false);
        self.base.request_redraw();
        true
    }
    impl_draw_bridge!();
}

impl Draw for MediaTransportBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let strip = self.strip();
        if strip.width == 0 || strip.height == 0 {
            return;
        }
        let style = self.base.style();
        let muted_ink = style.text_color.unwrap_or(Color::rgba(200, 200, 200, 230));
        let accent =
            self.accent.or(style.background_color).unwrap_or(Color::rgba(80, 150, 230, 240));
        let font = &role_font(style, 12.0);

        context.fill_rect(strip, Color::rgba(0, 0, 0, 160));
        if !self.visible {
            return;
        }

        let button = strip.height.saturating_sub(8).min(24);
        let button_y = strip.y + (strip.height as i32 - button as i32) / 2;
        let mut left = strip.x + 4;
        if self.show_next_previous {
            let previous = Rect::new(left, button_y, button, button);
            left = previous.right() + 6;
            draw_icon_at(context, previous, Color::WHITE, IconName::ArrowLeft);
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
            Color::WHITE,
            if self.current_playing() { IconName::Pause } else { IconName::Play },
        );
        if self.show_next_previous {
            let next = Rect::new(left, button_y, button, button);
            draw_icon_at(context, next, Color::WHITE, IconName::ArrowRight);
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
            draw_icon_at(context, rect, Color::WHITE, IconName::Fullscreen);
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
            draw_icon_at(context, rect, Color::WHITE, icon);
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

        // What remains is the track.
        let track_x = play.right() + 8;
        let track_width = (right - track_x).max(0) as u32;
        let track_y = strip.y + strip.height as i32 / 2 - 2;
        let track = Rect::new(track_x, track_y, track_width, 4);
        self.track_rect = Some(track);
        context.fill_rounded_rect(track, 2, Color::rgba(100, 100, 100, 200));
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
                self.notify_pointer_activity();
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
                    let volume = if muted { 0.0 } else { self.current_volume() };
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
    use crate::render::PaintBackend;

    #[test]
    fn a_press_toggles_the_playing_request() {
        let mut bar = MediaTransportBar::new(Rect::new(0, 0, 320, 40));
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = seen.clone();
        bar.play_toggled.connect(move |value: std::sync::Arc<bool>| {
            sink.lock().unwrap().push(*value);
        });
        // Paint once so the button rectangles are known.
        let mut backend = crate::render::SoftwarePaintBackend::new(Size::new(320, 40), 1.0);
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
        let mut backend = crate::render::SoftwarePaintBackend::new(Size::new(320, 40), 1.0);
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
}
