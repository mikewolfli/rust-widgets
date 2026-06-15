//! VideoPlayer — video player widget with play/pause/seek/volume controls.
//!
//! The VideoPlayer widget provides a video player interface with a placeholder
//! video area, transport controls (play/pause, seek bar, volume), time display,
//! and fullscreen button. It emits signals for playback state changes and time
//! updates.

use crate::core::{HorizontalAlignment, Color, Font, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// VideoPlayer — a video player widget with playback controls.
pub struct VideoPlayer {
    base: BaseWidget,
    /// Source URI or path of the video.
    source: String,
    /// Whether playback is active.
    is_playing: bool,
    /// Current playback position in seconds.
    current_time: f64,
    /// Total duration in seconds.
    duration: f64,
    /// Volume level (0.0 = silent, 1.0 = full).
    volume: f32,
    /// Whether audio is muted.
    muted: bool,
    /// Playback rate multiplier.
    playback_rate: f32,
    /// Whether the control bar is visible.
    controls_visible: bool,
    /// Emitted when playback starts.
    pub playback_started: GenericSignal,
    /// Emitted when playback is paused.
    pub playback_paused: GenericSignal,
    /// Emitted when playback reaches the end.
    pub playback_ended: GenericSignal,
    /// Emitted when the current playback time updates. Passes the new time.
    pub time_updated: Signal1<f64>,
}

impl VideoPlayer {
    /// Creates a new VideoPlayer with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::VideoPlayer, geometry, "VideoPlayer"),
            source: String::new(),
            is_playing: false,
            current_time: 0.0,
            duration: 0.0,
            volume: 0.8,
            muted: false,
            playback_rate: 1.0,
            controls_visible: true,
            playback_started: GenericSignal::new(),
            playback_paused: GenericSignal::new(),
            playback_ended: GenericSignal::new(),
            time_updated: Signal1::new(),
        }
    }

    /// Loads a video source by URI or file path.
    pub fn load(&mut self, source: &str) {
        self.source = source.to_string();
        self.current_time = 0.0;
        self.is_playing = false;
        self.base.request_redraw();
    }

    /// Returns the source URI.
    pub fn source(&self) -> &str {
        &self.source
    }

    /// Sets the video duration in seconds.
    pub fn set_duration(&mut self, duration: f64) {
        self.duration = duration.max(0.0);
        self.base.request_redraw();
    }

    /// Starts playback.
    pub fn play(&mut self) {
        if self.source.is_empty() {
            return;
        }
        if !self.is_playing {
            self.is_playing = true;
            self.playback_started.emit();
            self.base.request_redraw();
        }
    }

    /// Pauses playback.
    pub fn pause(&mut self) {
        if self.is_playing {
            self.is_playing = false;
            self.playback_paused.emit();
            self.base.request_redraw();
        }
    }

    /// Toggles between play and pause.
    pub fn toggle_play(&mut self) {
        if self.is_playing {
            self.pause();
        } else {
            self.play();
        }
    }

    /// Returns whether playback is active.
    pub fn is_playing(&self) -> bool {
        self.is_playing
    }

    /// Seeks to the given time in seconds (clamped to 0..duration).
    pub fn seek(&mut self, time: f64) {
        self.current_time = time.clamp(0.0, self.duration);
        self.time_updated.emit(self.current_time);
        self.base.request_redraw();
    }

    /// Returns the current playback time in seconds.
    pub fn current_time(&self) -> f64 {
        self.current_time
    }

    /// Returns the total duration in seconds.
    pub fn duration(&self) -> f64 {
        self.duration
    }

    /// Sets the volume level (0.0 = silent, 1.0 = full).
    pub fn set_volume(&mut self, vol: f32) {
        self.volume = vol.clamp(0.0, 1.0);
        self.base.request_redraw();
    }

    /// Returns the current volume level.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Sets whether audio is muted.
    pub fn set_muted(&mut self, muted: bool) {
        self.muted = muted;
        self.base.request_redraw();
    }

    /// Returns whether audio is muted.
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Sets the playback rate multiplier.
    pub fn set_playback_rate(&mut self, rate: f32) {
        self.playback_rate = rate.max(0.1);
        self.base.request_redraw();
    }

    /// Returns the current playback rate.
    pub fn playback_rate(&self) -> f32 {
        self.playback_rate
    }

    /// Shows the control bar overlay.
    pub fn show_controls(&mut self) {
        self.controls_visible = true;
        self.base.request_redraw();
    }

    /// Hides the control bar overlay.
    pub fn hide_controls(&mut self) {
        self.controls_visible = false;
        self.base.request_redraw();
    }

    /// Returns whether controls are visible.
    pub fn controls_visible(&self) -> bool {
        self.controls_visible
    }

    /// Advances playback time by the given number of seconds.
    /// This simulates video frame advancement for testing.
    /// Returns true if playback reached the end.
    pub fn tick(&mut self, delta_secs: f64) -> bool {
        if !self.is_playing || self.duration <= 0.0 {
            return false;
        }

        let effective_delta = delta_secs * self.playback_rate as f64;
        self.current_time += effective_delta;

        if self.current_time >= self.duration {
            self.current_time = self.duration;
            self.is_playing = false;
            self.playback_ended.emit();
            self.time_updated.emit(self.current_time);
            self.base.request_redraw();
            return true;
        }

        self.time_updated.emit(self.current_time);
        self.base.request_redraw();
        false
    }

    /// Returns the current playback progress as a fraction (0.0 – 1.0).
    pub fn progress(&self) -> f64 {
        if self.duration > 0.0 {
            (self.current_time / self.duration).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

impl Widget for VideoPlayer {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
}

impl Draw for VideoPlayer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // ── Video area ──────────────────────────────────────────────────
        let video_bg = if !is_enabled {
            Color::rgba(200, 200, 200, 100)
        } else if self.source.is_empty() {
            Color::rgba(30, 30, 30, 255)
        } else {
            Color::rgba(20, 20, 40, 255)
        };
        context.fill_rect(rect, video_bg);
        context.draw_rect_stroke(rect, Color::rgba(80, 80, 80, 200), 1);

        let font = Font::default();

        if self.source.is_empty() {
            // Empty state.
            let text = "No video loaded";
            let metrics = context.measure_text(text, &font);
            let text_x = rect.x + (rect.width as i32 - metrics.width as i32) / 2;
            let text_y = rect.y + rect.height as i32 / 2 + metrics.ascent as i32 / 2;
            context.draw_text(
                Point::new(text_x, text_y),
                text,
                &font,
                Color::rgba(180, 180, 180, 220),
                HorizontalAlignment::Left,
            );
            return;
        }

        // Play icon overlay in center when paused.
        if !self.is_playing {
            let play_icon = "▶";
            let icon_metrics = context.measure_text(play_icon, &font);
            let icon_x = rect.x + (rect.width as i32 - icon_metrics.width as i32) / 2;
            let icon_y = rect.y + rect.height as i32 / 2 + icon_metrics.ascent as i32 / 2;
            context.draw_text(
                Point::new(icon_x, icon_y),
                play_icon,
                &font,
                Color::rgba(255, 255, 255, 180),
                HorizontalAlignment::Left,
            );
        }

        // ── Control bar ─────────────────────────────────────────────────
        if !self.controls_visible {
            return;
        }

        let control_bar_height = 36u32;
        let control_bar_y = rect.y + rect.height as i32 - control_bar_height as i32;
        let control_bar_rect = Rect::new(rect.x, control_bar_y, rect.width, control_bar_height);
        context.fill_rect(control_bar_rect, Color::rgba(0, 0, 0, 160));

        // Play/Pause button.
        let btn_text = if self.is_playing { "⏸" } else { "▶" };
        let btn_metrics = context.measure_text(btn_text, &font);
        let btn_x = rect.x + 8;
        let btn_y = control_bar_y
            + (control_bar_height as i32 - btn_metrics.height as i32) / 2
            + btn_metrics.ascent as i32;
        context.draw_text(Point::new(btn_x, btn_y), btn_text, &font, Color::WHITE, HorizontalAlignment::Left);

        // Seek bar.
        let seek_bar_x = btn_x + btn_metrics.width as i32 + 12;
        let seek_bar_y = control_bar_y + control_bar_height as i32 / 2 - 4;
        let seek_bar_width = rect.width.saturating_sub((seek_bar_x - rect.x + 80) as u32);
        let seek_bar_height = 8u32;
        let seek_bar_full = Rect::new(seek_bar_x, seek_bar_y, seek_bar_width, seek_bar_height);
        context.fill_rounded_rect(seek_bar_full, 4, Color::rgba(100, 100, 100, 200));

        let fill_width = (seek_bar_full.width as f64 * self.progress()) as u32;
        if fill_width > 0 {
            let seek_bar_fill =
                Rect::new(seek_bar_full.x, seek_bar_full.y, fill_width, seek_bar_full.height);
            context.fill_rounded_rect(seek_bar_fill, 4, Color::rgba(60, 140, 240, 230));
        }

        // Time display.
        let time_text = format!(
            "{:02}:{:02}/{:02}:{:02}",
            (self.current_time as u32) / 60,
            (self.current_time as u32) % 60,
            (self.duration as u32) / 60,
            (self.duration as u32) % 60,
        );
        let time_metrics = context.measure_text(&time_text, &font);
        let time_x = seek_bar_full.x + seek_bar_full.width as i32 + 4;
        let time_y = control_bar_y
            + (control_bar_height as i32 - time_metrics.height as i32) / 2
            + time_metrics.ascent as i32;
        context.draw_text(
            Point::new(time_x, time_y),
            &time_text,
            &font,
            Color::rgba(220, 220, 220, 230),
            HorizontalAlignment::Left,
        );

        // Volume icon and mute button.
        let vol_text = if self.muted || self.volume < 0.01 {
            "🔇"
        } else if self.volume < 0.5 {
            "🔉"
        } else {
            "🔊"
        };
        let vol_metrics = context.measure_text(vol_text, &font);
        let vol_x = rect.x + rect.width as i32 - vol_metrics.width as i32 - 8;
        let vol_y = control_bar_y
            + (control_bar_height as i32 - vol_metrics.height as i32) / 2
            + vol_metrics.ascent as i32;
        context.draw_text(Point::new(vol_x, vol_y), vol_text, &font, Color::WHITE, HorizontalAlignment::Left);

        // Playback rate indicator.
        if (self.playback_rate - 1.0).abs() > 0.01 {
            let rate_text = format!("{:.1}x", self.playback_rate);
            let rate_metrics = context.measure_text(&rate_text, &font);
            let rate_x = vol_x - rate_metrics.width as i32 - 8;
            let rate_y = control_bar_y
                + (control_bar_height as i32 - rate_metrics.height as i32) / 2
                + rate_metrics.ascent as i32;
            context.draw_text(
                Point::new(rate_x, rate_y),
                &rate_text,
                &font,
                Color::rgba(255, 220, 100, 230),
                HorizontalAlignment::Left,
            );
        }

        // Fullscreen button.
        let fs_text = "⛶";
        let fs_metrics = context.measure_text(fs_text, &font);
        let fs_x = vol_x - fs_metrics.width as i32 - 24;
        let fs_y = control_bar_y
            + (control_bar_height as i32 - fs_metrics.height as i32) / 2
            + fs_metrics.ascent as i32;
        context.draw_text(Point::new(fs_x, fs_y), fs_text, &font, Color::WHITE, HorizontalAlignment::Left);
    }
}

impl EventHandler for VideoPlayer {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    let rect = self.geometry();
                    if !rect.contains_point(*pos) {
                        return;
                    }

                    // Check if clicked on the play/pause button area.
                    let control_bar_height = 36u32;
                    let control_bar_y = rect.y + rect.height as i32 - control_bar_height as i32;

                    if pos.y >= control_bar_y
                        && pos.y < rect.y + rect.height as i32
                        && self.controls_visible
                    {
                        // Play/pause button click area.
                        let font = Font::default();
                        let btn_text = if self.is_playing { "⏸" } else { "▶" };
                        let btn_metrics = context::private::measure_text_static(&font, btn_text);
                        let btn_x = rect.x + 8;
                        let btn_w = btn_metrics.width as i32 + 4;
                        let btn_h = btn_metrics.height as i32 + 4;
                        let btn_rect = Rect::new(btn_x, control_bar_y, btn_w as u32, btn_h as u32);
                        if btn_rect.contains_point(*pos) {
                            self.toggle_play();
                            return;
                        }
                    }

                    // Click on video area toggles controls visibility or play/pause.
                    if pos.y < control_bar_y {
                        if self.controls_visible {
                            self.toggle_play();
                        } else {
                            self.show_controls();
                        }
                    }
                }
            }
            Event::KeyPress { key, modifiers } => {
                match (key, modifiers) {
                    // Space bar toggles play/pause.
                    (32, _) => {
                        self.toggle_play();
                    }
                    // Left arrow seeks backward 5 seconds.
                    (37, _) => {
                        self.seek(self.current_time - 5.0);
                    }
                    // Right arrow seeks forward 5 seconds.
                    (39, _) => {
                        self.seek(self.current_time + 5.0);
                    }
                    // Up arrow increases volume.
                    (38, _) => {
                        self.set_volume(self.volume + 0.1);
                    }
                    // Down arrow decreases volume.
                    (40, _) => {
                        self.set_volume(self.volume - 0.1);
                    }
                    // M key toggles mute.
                    (109, _) | (77, _) => {
                        self.set_muted(!self.muted);
                    }
                    _ => {}
                }
            }
            _ => {
                self.base.handle_event(event);
            }
        }
    }
}

/// Internal helper module for static text metrics (avoiding RenderContext dependency in event handler).
mod context {
    pub mod private {
        use crate::core::Font;
        use crate::render::TextMetrics;
        pub fn measure_text_static(_font: &Font, text: &str) -> TextMetrics {
            // Simple approximation: width ≈ chars * 8, height = 16
            TextMetrics { width: (text.len() as u32) * 8, height: 16, ascent: 12, descent: 4 }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn video_player_creation_defaults() {
        let vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        assert_eq!(vp.source(), "");
        assert!(!vp.is_playing());
        assert_eq!(vp.current_time(), 0.0);
        assert_eq!(vp.duration(), 0.0);
        assert_eq!(vp.volume(), 0.8);
        assert!(!vp.is_muted());
        assert_eq!(vp.playback_rate(), 1.0);
        assert!(vp.controls_visible());
        assert_eq!(vp.kind(), WidgetKind::VideoPlayer);
    }

    #[test]
    fn video_player_load_play_pause_toggle() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test_video.mp4");
        assert_eq!(vp.source(), "test_video.mp4");

        assert!(!vp.is_playing());
        vp.play();
        assert!(vp.is_playing());
        vp.pause();
        assert!(!vp.is_playing());

        vp.toggle_play();
        assert!(vp.is_playing());
        vp.toggle_play();
        assert!(!vp.is_playing());
    }

    #[test]
    fn video_player_seek_and_time() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test.mp4");
        vp.set_duration(120.0);

        assert_eq!(vp.current_time(), 0.0);
        vp.seek(60.0);
        assert_eq!(vp.current_time(), 60.0);
        vp.seek(200.0); // clamped
        assert_eq!(vp.current_time(), 120.0);
        vp.seek(-10.0); // clamped
        assert_eq!(vp.current_time(), 0.0);
    }

    #[test]
    fn video_player_volume_and_mute() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        assert_eq!(vp.volume(), 0.8);
        assert!(!vp.is_muted());

        vp.set_volume(0.5);
        assert_eq!(vp.volume(), 0.5);

        vp.set_volume(2.0); // clamped
        assert_eq!(vp.volume(), 1.0);

        vp.set_volume(-1.0); // clamped
        assert_eq!(vp.volume(), 0.0);

        vp.set_muted(true);
        assert!(vp.is_muted());
        vp.set_muted(false);
        assert!(!vp.is_muted());
    }

    #[test]
    fn video_player_playback_rate() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        assert_eq!(vp.playback_rate(), 1.0);
        vp.set_playback_rate(2.0);
        assert_eq!(vp.playback_rate(), 2.0);
        vp.set_playback_rate(0.0); // clamped
        assert_eq!(vp.playback_rate(), 0.1);
    }

    #[test]
    fn video_player_controls_visibility() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        assert!(vp.controls_visible());
        vp.hide_controls();
        assert!(!vp.controls_visible());
        vp.show_controls();
        assert!(vp.controls_visible());
    }

    #[test]
    fn video_player_progress() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_duration(100.0);
        assert_eq!(vp.progress(), 0.0);
        vp.seek(50.0);
        assert!((vp.progress() - 0.5).abs() < 0.001);
        vp.seek(100.0);
        assert!((vp.progress() - 1.0).abs() < 0.001);
    }

    #[test]
    fn video_player_tick_advances_time() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test.mp4");
        vp.set_duration(60.0);
        vp.play();

        assert_eq!(vp.current_time(), 0.0);
        vp.tick(10.0);
        assert!((vp.current_time() - 10.0).abs() < 0.001);
        vp.tick(20.0);
        assert!((vp.current_time() - 30.0).abs() < 0.001);
    }

    #[test]
    fn video_player_tick_playback_ended() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test.mp4");
        vp.set_duration(10.0);
        vp.play();

        let ended = Arc::new(Mutex::new(false));
        vp.playback_ended.connect({
            let ended = Arc::clone(&ended);
            move || {
                *ended.lock().unwrap() = true;
            }
        });

        let finished = vp.tick(15.0);
        assert!(finished);
        assert!(*ended.lock().unwrap());
        assert!(!vp.is_playing());
        assert_eq!(vp.current_time(), 10.0);
    }

    #[test]
    fn video_player_signals() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test.mp4");
        vp.set_duration(30.0);

        let started = Arc::new(Mutex::new(false));
        let paused = Arc::new(Mutex::new(false));
        let last_time = Arc::new(Mutex::new(0.0));

        vp.playback_started.connect({
            let started = Arc::clone(&started);
            move || {
                *started.lock().unwrap() = true;
            }
        });
        vp.playback_paused.connect({
            let paused = Arc::clone(&paused);
            move || {
                *paused.lock().unwrap() = true;
            }
        });
        vp.time_updated.connect({
            let last_time = Arc::clone(&last_time);
            move |val: Arc<f64>| {
                *last_time.lock().unwrap() = *val;
            }
        });

        vp.play();
        assert!(*started.lock().unwrap());

        vp.seek(15.0);
        assert!((*last_time.lock().unwrap() - 15.0).abs() < 0.001);

        vp.pause();
        assert!(*paused.lock().unwrap());
    }
}
