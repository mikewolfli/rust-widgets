//! MediaPlayer widget.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Lightweight media player stateful control.
pub struct MediaPlayer {
    base: BaseWidget,
    source: Option<String>,
    playing: bool,
    duration_ms: u64,
    position_ms: u64,
    volume: u8,
    muted: bool,
    fullscreen: bool,
    /// Emitted when playback toggles.
    pub playback_changed: Signal1<bool>,
    /// Emitted when position changes.
    pub position_changed: Signal1<u64>,
    /// Emitted when volume changes.
    pub volume_changed: Signal1<u8>,
    /// Emitted when source changes.
    pub source_changed: Signal1<String>,
}

impl MediaPlayer {
    /// Creates empty media player.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::WebEngineView, geometry, "MediaPlayer"),
            source: None,
            playing: false,
            duration_ms: 0,
            position_ms: 0,
            volume: 80,
            muted: false,
            fullscreen: false,
            playback_changed: Signal1::new(),
            position_changed: Signal1::new(),
            volume_changed: Signal1::new(),
            source_changed: Signal1::new(),
        }
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
        self.position_ms = 0;
        self.playing = false;
        self.source_changed.emit(source);
        self.playback_changed.emit(false);
        self.position_changed.emit(0);
        self.base.request_redraw();
    }

    /// Clears loaded source.
    pub fn clear_source(&mut self) {
        self.source = None;
        self.duration_ms = 0;
        self.position_ms = 0;
        self.playing = false;
        self.playback_changed.emit(false);
        self.position_changed.emit(0);
        self.base.request_redraw();
    }

    /// Returns whether playback is active.
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Returns duration in milliseconds.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }

    /// Returns current playback position in milliseconds.
    pub fn position_ms(&self) -> u64 {
        self.position_ms
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
        if !self.playing {
            self.playing = true;
            self.playback_changed.emit(true);
            self.base.request_redraw();
        }
        true
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        if self.playing {
            self.playing = false;
            self.playback_changed.emit(false);
            self.base.request_redraw();
        }
    }

    /// Toggles play/pause.
    pub fn toggle_playback(&mut self) -> bool {
        if self.playing {
            self.pause();
            true
        } else {
            self.play()
        }
    }

    /// Seeks to absolute position.
    pub fn seek_to(&mut self, position_ms: u64) {
        let max_pos = self.duration_ms;
        let next = position_ms.min(max_pos);
        if next != self.position_ms {
            self.position_ms = next;
            self.position_changed.emit(self.position_ms);
            self.base.request_redraw();
        }
    }

    /// Seeks by signed delta milliseconds.
    pub fn seek_by(&mut self, delta_ms: i64) {
        let current = self.position_ms as i64;
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
            self.base.request_redraw();
        }
    }

    /// Toggles fullscreen state.
    pub fn toggle_fullscreen(&mut self) {
        self.set_fullscreen(!self.fullscreen);
    }

    fn progress_ratio(&self) -> f32 {
        if self.duration_ms == 0 {
            return 0.0;
        }
        (self.position_ms as f32 / self.duration_ms as f32).clamp(0.0, 1.0)
    }
}

impl Widget for MediaPlayer {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, 240)
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
                // Unknown key; ignore
                _ => {}
            },
            Event::MousePress { pos, button: 1 } => {
                let rect = self.geometry();
                let bar_rect = Rect::new(
                    rect.x + 10,
                    rect.y + rect.height as i32 - 18,
                    rect.width.saturating_sub(20),
                    8,
                );
                if pos.x >= bar_rect.x
                    && pos.x < bar_rect.x + bar_rect.width as i32
                    && pos.y >= bar_rect.y
                    && pos.y < bar_rect.y + bar_rect.height as i32
                {
                    if self.duration_ms > 0 {
                        let ratio =
                            ((pos.x - bar_rect.x) as f32 / bar_rect.width as f32).clamp(0.0, 1.0);
                        self.seek_to((ratio * self.duration_ms as f32) as u64);
                    }
                } else {
                    let _ = self.toggle_playback();
                }
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for MediaPlayer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        context.fill_rect(rect, Color::rgb(24, 28, 36));
        context.draw_rect(rect, Color::rgb(72, 84, 102));

        let title = self
            .source
            .as_deref()
            .map(|src| src.rsplit('/').next().unwrap_or(src))
            .unwrap_or("No media");
        let state = if self.playing { "Playing" } else { "Paused" };
        let vol = if self.muted { "Muted".to_string() } else { format!("Vol {}", self.volume) };
        let fs = if self.fullscreen { "Fullscreen" } else { "Window" };

        context.draw_text(
            Point::new(rect.x + 10, rect.y + 18),
            title,
            &Font::default(),
            Color::rgb(232, 237, 245),
            HorizontalAlignment::Left,
        );
        context.draw_text(
            Point::new(rect.x + 10, rect.y + 36),
            &format!("{} | {} | {}", state, vol, fs),
            &Font::default(),
            Color::rgb(190, 202, 220),
            HorizontalAlignment::Left,
        );

        let bar_rect = Rect::new(
            rect.x + 10,
            rect.y + rect.height as i32 - 18,
            rect.width.saturating_sub(20),
            8,
        );
        context.fill_rect(bar_rect, Color::rgb(62, 73, 90));
        let fill_w = ((bar_rect.width as f32) * self.progress_ratio()) as u32;
        if fill_w > 0 {
            context.fill_rect(
                Rect::new(bar_rect.x, bar_rect.y, fill_w, bar_rect.height),
                Color::rgb(107, 171, 248),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

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
