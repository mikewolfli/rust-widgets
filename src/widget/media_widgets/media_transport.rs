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

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
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
    accent: Option<Color>,
    /// Whether the progress track is being dragged.
    scrubbing: bool,
    /// The progress track from the last paint, so a drag maps to the painted span.
    track_rect: Option<Rect>,
    /// The play/pause button from the last paint.
    play_rect: Option<Rect>,
    /// The volume button from the last paint.
    volume_rect: Option<Rect>,
    /// The fullscreen button from the last paint.
    fullscreen_rect: Option<Rect>,
    /// Emitted with the new desired playing state.
    pub play_toggled: Signal1<bool>,
    /// Emitted with the requested position as a `0.0..=1.0` fraction.
    pub seek_requested: Signal1<f32>,
    /// Emitted with the requested volume, `0.0..=1.0`.
    pub volume_changed: Signal1<f32>,
    /// Emitted when the fullscreen control is activated.
    pub fullscreen_requested: GenericSignal,
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
            accent: None,
            scrubbing: false,
            track_rect: None,
            play_rect: None,
            volume_rect: None,
            fullscreen_rect: None,
            play_toggled: Signal1::new(),
            seek_requested: Signal1::new(),
            volume_changed: Signal1::new(),
            fullscreen_requested: GenericSignal::new(),
        }
    }

    /// Binds a video player, adopting its current playback state.
    pub fn bind_video_player(&mut self, player: &mut crate::widget::media_widgets::VideoPlayer) {
        self.target_kind = MediaTargetKind::VideoPlayer;
        self.playing = player.is_playing();
        self.position_ms = (player.current_time() * 1000.0) as u64;
        self.duration_ms = (player.duration() * 1000.0) as u64;
        self.volume = player.volume();
        self.muted = player.is_muted();
        self.base.request_redraw();
    }

    /// Binds an animated image, adopting its current playback state.
    pub fn bind_animated_image(&mut self, image: &mut crate::widget::media_widgets::AnimatedImage) {
        self.target_kind = MediaTargetKind::AnimatedImage;
        self.playing = image.is_playing();
        self.base.request_redraw();
    }

    /// Binds a Lottie animation, adopting its current playback state.
    pub fn bind_lottie(&mut self, lottie: &mut crate::widget::media_widgets::LottieWidget) {
        self.target_kind = MediaTargetKind::LottieWidget;
        self.playing = lottie.is_playing();
        self.base.request_redraw();
    }

    /// Binds a Rive animation, adopting its current playback state.
    pub fn bind_rive(&mut self, rive: &mut crate::widget::media_widgets::RiveWidget) {
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
        if self.playing == playing {
            return;
        }
        self.playing = playing;
        self.base.request_redraw();
    }

    /// Whether the bar believes playback is active.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Sets the position and duration shown by the track.
    pub fn set_position(&mut self, position_ms: u64, duration_ms: u64) {
        if self.position_ms == position_ms && self.duration_ms == duration_ms {
            return;
        }
        self.position_ms = position_ms;
        self.duration_ms = duration_ms;
        self.base.request_redraw();
    }

    /// The shown position and duration in milliseconds.
    pub fn position(&self) -> (u64, u64) {
        (self.position_ms, self.duration_ms)
    }

    /// Sets the volume, clamped to `0.0..=1.0`.
    pub fn set_volume(&mut self, volume: f32) {
        let volume = if volume.is_finite() { volume.clamp(0.0, 1.0) } else { return };
        if (self.volume - volume).abs() < f32::EPSILON {
            return;
        }
        self.volume = volume;
        self.base.request_redraw();
    }

    /// The shown volume.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Hides or shows the volume control.
    pub fn set_show_volume(&mut self, show: bool) {
        if self.show_volume == show {
            return;
        }
        self.show_volume = show;
        self.base.request_redraw();
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
        if self.duration_ms == 0 {
            0.0
        } else {
            (self.position_ms as f32 / self.duration_ms as f32).clamp(0.0, 1.0)
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
        self.position_ms = (self.duration_ms as f32 * fraction) as u64;
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
        let play = Rect::new(
            strip.x + 4,
            strip.y + (strip.height as i32 - button as i32) / 2,
            button,
            button,
        );
        self.play_rect = Some(play);
        draw_icon_at(
            context,
            play,
            Color::WHITE,
            if self.playing { IconName::Pause } else { IconName::Play },
        );

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
            let icon = if self.muted || self.volume < 0.01 {
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
            let text = timecode(self.position_ms, self.duration_ms);
            let metrics = context.measure_text(&text, font);
            let x = right - metrics.width as i32 - 6;
            let y = strip.y + (strip.height as i32 - metrics.height as i32) / 2;
            context.draw_text(Point::new(x, y), &text, font, muted_ink, HorizontalAlignment::Left);
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
                if self.play_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    self.playing = !self.playing;
                    self.play_toggled.emit(self.playing);
                    self.base.request_redraw();
                    return;
                }
                if self.fullscreen_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    self.fullscreen_requested.emit();
                    return;
                }
                if self.volume_rect.is_some_and(|rect| rect.contains_point(*pos)) {
                    self.muted = !self.muted;
                    let volume = if self.muted { 0.0 } else { self.volume };
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
}
