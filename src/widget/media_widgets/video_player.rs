// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! VideoPlayer — simulated video player widget (self-drawn UI, no media engine).
//!
//! This is a simulation/preview component: it draws a player UI (video area
//! background, transport controls, seek bar, time display) and tracks a local
//! simulated clock. It does not open, demux or decode media files. play/pause/
//! seek only move the simulated position, which advances when `tick` is called.
//! It emits signals for playback state changes and time updates.

use core::fmt::Write as _;

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::impl_widget_property_hooks;
use crate::property_names_of;
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};
use crate::widget::capability::coercion::{expect_bool, expect_f32};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};

/// VideoPlayer — simulated video player widget with playback controls.
///
/// Self-drawn UI only: no media is decoded or rendered. State (play/pause,
/// position, volume) is simulated and displayed locally.
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

    /// Records a video source URI or file path for display.
    ///
    /// The source is stored as a label only: it is not opened, demuxed or
    /// decoded. Loading also resets the simulated clock and pauses playback.
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

    /// Sets the simulated video duration in seconds used by the clock, seek bar
    /// and time display. The value comes from the caller; it is not measured
    /// from any media file. Non-finite values are ignored. If the duration is
    /// shortened below the current position, the clock is clamped and emits
    /// `time_updated` with the new position.
    pub fn set_duration(&mut self, duration: f64) {
        if !duration.is_finite() {
            return;
        }
        let duration = duration.max(0.0);
        if self.duration == duration {
            return;
        }
        self.duration = duration;
        let clamped_time = self.current_time.clamp(0.0, self.duration);
        if clamped_time != self.current_time {
            self.current_time = clamped_time;
            self.time_updated.emit(self.current_time);
        }
        self.base.request_redraw();
    }

    /// Starts simulated playback.
    ///
    /// Marks the player as playing and emits `playback_started`. No media
    /// frames are decoded; the position advances only when `tick` is called.
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

    /// Seeks the simulated playback clock to the given time in seconds
    /// (clamped to 0..duration). Does not read any media data.
    ///
    /// A non-finite request (`NaN`, `±inf`) has no position on the timeline; it is ignored so it
    /// cannot poison `current_time`, which every later `progress` / `tick` / `time_updated` reads.
    pub fn seek(&mut self, time: f64) {
        if !time.is_finite() {
            return;
        }
        self.current_time = time.clamp(0.0, self.duration.max(0.0));
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
        let volume = vol.clamp(0.0, 1.0);
        if self.volume == volume {
            return;
        }
        self.volume = volume;
        self.base.request_redraw();
    }

    /// Returns the current volume level.
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Sets whether audio is muted.
    pub fn set_muted(&mut self, muted: bool) {
        if self.muted == muted {
            return;
        }
        self.muted = muted;
        self.base.request_redraw();
    }

    /// Returns whether audio is muted.
    pub fn is_muted(&self) -> bool {
        self.muted
    }

    /// Sets the playback rate multiplier.
    pub fn set_playback_rate(&mut self, rate: f32) {
        if !rate.is_finite() {
            return;
        }
        let rate = rate.max(0.1);
        if self.playback_rate == rate {
            return;
        }
        self.playback_rate = rate;
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

    /// Advances the simulated playback clock by the given number of seconds
    /// (scaled by playback rate).
    /// This simulates video frame advancement; no media is decoded.
    /// Returns true if playback reached the end.
    ///
    /// A non-finite or negative `delta_secs` is rejected: a clock that runs on `NaN` never satisfies
    /// `current_time >= duration` (all comparisons with `NaN` are false), so the control would keep
    /// emitting `time_updated` with a `NaN` payload forever, and a negative delta would run the
    /// clock backwards past zero. Both are input errors with no timeline position, so the tick is a
    /// no-op that reports "not finished" rather than corrupting the state.
    pub fn tick(&mut self, delta_secs: f64) -> bool {
        if !self.is_playing || self.duration <= 0.0 || !delta_secs.is_finite() || delta_secs < 0.0 {
            return false;
        }

        let effective_delta = delta_secs * self.playback_rate as f64;
        // `playback_rate` is a finite `f32` clamped positive, so `effective_delta` stays finite; the
        // clamp below is the belt-and-braces that keeps `current_time` in `[0, duration]` regardless.
        self.current_time = (self.current_time + effective_delta).clamp(0.0, self.duration);

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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(320, 240)
    }

    /// Reports this widget as the object that paints it.
    ///
    /// `VideoPlayer` implements `Draw`, so `Some(self)` is total and cannot be
    /// wrong.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    impl_widget_property_hooks!();
    // Trait contract: `u32` ms. This control's own `tick` takes seconds as `f64` because
    // playback position is a media clock rather than a frame delta; convert at the boundary.
    fn tick(&mut self, delta_ms: u32) -> bool {
        VideoPlayer::tick(self, f64::from(delta_ms) / 1000.0)
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
            "playback_started" => {
                Some(EventSignalRef::unit("playback_started", &self.playback_started))
            }
            "playback_paused" => {
                Some(EventSignalRef::unit("playback_paused", &self.playback_paused))
            }
            "playback_ended" => Some(EventSignalRef::unit("playback_ended", &self.playback_ended)),
            "time_updated" => {
                Some(EventSignalRef::mapped("time_updated", &self.time_updated, |v| {
                    CapabilityValue::Float(*v)
                }))
            }
            _ => None,
        }
    }
}

/// `VideoPlayer`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_media.in.rs` / `access_write_media.in.rs` dispatch.
impl WidgetProperties for VideoPlayer {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "is_playing" => Ok(CapabilityValue::Bool(self.is_playing())),
            "volume" => Ok(CapabilityValue::Float(self.volume() as f64)),
            "muted" => Ok(CapabilityValue::Bool(self.is_muted())),
            "playback_rate" => Ok(CapabilityValue::Float(self.playback_rate() as f64)),
            "controls_visible" => Ok(CapabilityValue::Bool(self.controls_visible())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "is_playing" => {
                if expect_bool(value)? {
                    self.play();
                } else {
                    self.pause();
                }
                Ok(())
            }
            "volume" => {
                self.set_volume(expect_f32(value)?);
                Ok(())
            }
            "muted" => {
                self.set_muted(expect_bool(value)?);
                Ok(())
            }
            "playback_rate" => {
                self.set_playback_rate(expect_f32(value)?);
                Ok(())
            }
            "controls_visible" => {
                if expect_bool(value)? {
                    self.show_controls();
                } else {
                    self.hide_controls();
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "is_playing",
            "volume",
            "muted",
            "playback_rate",
            "controls_visible",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl Draw for VideoPlayer {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        let is_enabled = self.base.is_enabled();

        // # Which of this control's colours are chrome and which are the video's own frame
        //
        // A media player deliberately draws its *transport* chrome in white on a black band:
        // the picture beneath is arbitrary, so an overlay that took the page's ink would be
        // unreadable over a light frame and disappear over a dark one. Those literals are kept,
        // and they are the reason `audit_appearance.py` still counts literals in this file.
        //
        // Two things are **not** the video's frame, and those follow the theme:
        //
        //   * the **empty state** -- there is no picture yet, so the rectangle is a placeholder
        //     panel, and a panel belongs to the page's surface scale (`surface_container`);
        //   * the **seek fill** -- a value indicator, so it is the accent, exactly as a slider's
        //     fill or a progress bar's is. It was a fixed `rgb(60,140,240)` that no theme could
        //     move.
        let (accent, placeholder, placeholder_ink, window_fill) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.primary,
                    active.colors.surface_container,
                    active.colors.secondary,
                    active.colors.background,
                ),
                None => (
                    Color::rgba(60, 140, 240, 230),
                    Color::rgba(30, 30, 30, 255),
                    Color::rgba(180, 180, 180, 220),
                    Color::WHITE,
                ),
            }
        };

        // ── Video area ──────────────────────────────────────────────────
        let video_bg = if !is_enabled {
            // A disabled player is the page's own surface, veiled, rather than a fixed
            // translucent grey that read as a light slab in a dark appearance.
            self.base.disabled_surface_near(placeholder, window_fill)
        } else if self.source.is_empty() {
            placeholder
        } else {
            Color::rgba(20, 20, 40, 255)
        };
        context.face_with_gradient(
            rect,
            video_bg,
            self.style().background_gradient.as_ref(),
            self.style().surface.unwrap_or_default(),
            self.style().border_radius.unwrap_or(0),
            Color::BLACK,
        );
        context.draw_rect_stroke(
            rect,
            self.style().border_color.unwrap_or_else(|| video_bg.blend(&Color::WHITE, 0.2)),
            1,
        );

        // The control's chrome text (placeholder, time, rate) takes the **effective font**, so the
        // theme body font and the user's text scale reach it (D09-STYLE-01).
        let font = effective_font(self.style());

        if self.source.is_empty() {
            // Empty state. Fitted to the control's width so a narrow player truncates the
            // label instead of drawing it past its own edge.
            let text = "No video loaded";
            let metrics = context.measure_text(text, font);
            let text_y = rect.y + (rect.height as i32 - metrics.height as i32) / 2;
            let line = Rect::new(rect.x, text_y, rect.width, metrics.height);
            context.draw_text_fitted_cow(
                line,
                text,
                font,
                placeholder_ink,
                HorizontalAlignment::Center,
            );
            return;
        }

        // Play icon overlay in center when paused — the `Play` **icon outline**, not the `▶`
        // text glyph (U+25B6 is covered by no bundled face, so it drew as an 8x8 bitmap block).
        if !self.is_playing {
            let side = (rect.height / 3).clamp(24, 96);
            crate::widget::draw_icon_centered(
                context,
                rect,
                side,
                Color::rgba(255, 255, 255, 180),
                IconName::Play,
            );
        }

        // ── Control bar ─────────────────────────────────────────────────
        if !self.controls_visible {
            return;
        }

        // The transport bar is a fixed overlay pinned to the video surface's bottom edge; its
        // height is the player's own fact, not a fraction of the surface it covers, so it stays
        // 36 px whether the player is a 320x240 panel or a full-screen canvas.
        let control_bar_height = dimensions::VIDEO_CONTROL_BAR_HEIGHT;
        let control_bar_rect = ControlMetrics::bottom_band(rect, control_bar_height);
        let control_bar_y = control_bar_rect.y;
        context.fill_rect(control_bar_rect, Color::rgba(0, 0, 0, 160));

        // Play/Pause button: a `Play`/`Pause` **icon outline**, not the `▶`/`⏸` text glyphs
        // (U+25B6 and U+23F8 are covered by no bundled face).
        let btn_icon = if self.is_playing { IconName::Pause } else { IconName::Play };
        let btn_side = 16u32;
        let btn_box = Rect::new(rect.x + 8, control_bar_y, btn_side, control_bar_height);
        crate::widget::draw_icon_centered(context, btn_box, btn_side, Color::WHITE, btn_icon);

        // Seek bar: the shared thickness, centred on the transport bar's own middle line.
        //
        // The right-hand items are laid out from the right edge first, then the time label
        // is placed left of them, and only then is the seek bar given what remains. The old
        // fixed 80 px reservation held an 11-glyph time string (≈74 px at 14 px) and left no
        // room for a playback-rate label, which was then drawn over the fullscreen icon.
        let seek_bar_x = rect.x + 8 + btn_side as i32 + 12;
        let seek_bar_y = control_bar_y
            + (control_bar_height as i32 - dimensions::VIDEO_SEEK_BAR_HEIGHT as i32) / 2;
        let seek_bar_height = dimensions::VIDEO_SEEK_BAR_HEIGHT;
        // Both labels render in place: `format!` built a fresh `String` for the timestamp
        // and another for the rate every frame.
        let mut time_text = String::new();
        let _ = write!(
            time_text,
            "{:02}:{:02}/{:02}:{:02}",
            (self.current_time as u32) / 60,
            (self.current_time as u32) % 60,
            (self.duration as u32) / 60,
            (self.duration as u32) % 60,
        );
        let time_metrics = context.measure_text(&time_text, font);
        let time_width = time_metrics.width as i32;
        let vol_side = 16u32;
        let vol_box = Rect::new(
            rect.x + rect.width as i32 - vol_side as i32 - 8,
            control_bar_y,
            vol_side,
            control_bar_height,
        );
        let fs_side = 16u32;
        let fs_box =
            Rect::new(vol_box.x - fs_side as i32 - 24, control_bar_y, fs_side, control_bar_height);
        // The rate label is measured once and the metrics reused for both the layout and
        // the draw; the old form measured it twice and allocated the text as an `Option<String>`.
        let has_rate = (self.playback_rate - 1.0).abs() > 0.01;
        let mut rate_text = String::new();
        if has_rate {
            let _ = write!(rate_text, "{:.1}x", self.playback_rate);
        }
        let rate_metrics =
            if has_rate { Some(context.measure_text(&rate_text, font)) } else { None };
        let rate_width = rate_metrics.as_ref().map(|m| m.width as i32).unwrap_or(0);
        // The rate label sits left of the fullscreen icon; the time label sits left of the
        // whole right-hand group. Both gaps are real, so neither label can overlap an icon.
        let rate_x = fs_box.x - rate_width - 8;
        let time_x = if has_rate { rate_x } else { fs_box.x } - time_width - 8;
        let seek_bar_width = (time_x - 8 - seek_bar_x).max(0) as u32;
        let seek_bar_full = Rect::new(seek_bar_x, seek_bar_y, seek_bar_width, seek_bar_height);
        context.fill_rounded_rect(
            seek_bar_full,
            seek_bar_height / 2,
            Color::rgba(100, 100, 100, 200),
        );

        let fill_width = (seek_bar_full.width as f64 * self.progress()) as u32;
        if fill_width > 0 {
            let seek_bar_fill =
                Rect::new(seek_bar_full.x, seek_bar_full.y, fill_width, seek_bar_full.height);
            context.fill_rounded_rect(seek_bar_fill, seek_bar_height / 2, accent);
        }

        // Time display, placed in the gap the right-hand group left for it.
        let time_y = control_bar_y + (control_bar_height as i32 - time_metrics.height as i32) / 2;
        context.draw_text(
            Point::new(time_x, time_y),
            &time_text,
            font,
            Color::rgba(220, 220, 220, 230),
            HorizontalAlignment::Left,
        );

        // Volume icon and mute button: a `VolumeOff`/`VolumeUp` **icon outline**, not the
        // `🔇`/`🔉`/`🔊` emoji (no bundled face covers them, and a colour emoji here is not what a
        // transport bar wants). Muted is the crossed speaker; otherwise the plain speaker.
        let vol_icon =
            if self.muted || self.volume < 0.01 { IconName::VolumeOff } else { IconName::VolumeUp };
        crate::widget::draw_icon_centered(context, vol_box, vol_side, Color::WHITE, vol_icon);

        // Playback rate indicator, in the slot reserved left of the fullscreen button.
        if let Some(metrics) = rate_metrics.as_ref() {
            let rate_y = control_bar_y + (control_bar_height as i32 - metrics.height as i32) / 2;
            context.draw_text(
                Point::new(rate_x, rate_y),
                &rate_text,
                font,
                Color::rgba(255, 220, 100, 230),
                HorizontalAlignment::Left,
            );
        }

        // Fullscreen button: a `Fullscreen` **icon outline**, not the `⛶` text glyph (U+26F6 is
        // covered by no bundled face).
        crate::widget::draw_icon_centered(
            context,
            fs_box,
            fs_side,
            Color::WHITE,
            IconName::Fullscreen,
        );
    }
}

impl EventHandler for VideoPlayer {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button, .. } => {
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
                        // Play/pause button click area. The box matches the drawn button: a
                        // 16 px icon in a strip at `x + 8`, full transport-bar height, so the hit
                        // target and the ink are the same object (a glyph-measured box here and a
                        // fixed icon box in `draw` would disagree on where the button is).
                        let btn_rect = Rect::new(rect.x + 8, control_bar_y, 16, control_bar_height);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// Renders the player over `backdrop` and returns its RGBA frame.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn render_on(vp: &mut VideoPlayer, size: crate::core::Size, backdrop: Color) -> Vec<u8> {
        use crate::render::{PaintBackend, SoftwarePaintBackend};
        let mut backend = SoftwarePaintBackend::new(size, 1.0);
        backend.begin_frame(backdrop);
        let mut context = RenderContext::new(&mut backend);
        vp.draw(&mut context);
        backend.end_frame();
        backend.frame_rgba().to_vec()
    }

    /// The RGBA pixel at `(x, y)` of a `size`-wide frame.
    #[cfg(all(device_profile, feature = "desktop"))]
    fn pixel(frame: &[u8], size: crate::core::Size, x: u32, y: u32) -> [u8; 4] {
        let index = ((y * size.width + x) * 4) as usize;
        [frame[index], frame[index + 1], frame[index + 2], frame[index + 3]]
    }

    /// The empty-state panel follows the appearance rather than a fixed near-black.
    ///
    /// # What is themed here, and what deliberately is not
    ///
    /// The transport chrome stays white on a black band on purpose: the picture beneath is
    /// arbitrary, so an overlay that took the page's ink would be unreadable over a light frame.
    /// The **empty state** is different — there is no picture, so the rectangle is a placeholder
    /// panel and belongs to the page's surface scale. It used to be a fixed `rgb(30,30,30)`, which on
    /// a light appearance was a black rectangle where every other placeholder in the crate is a
    /// surface.
    #[test]
    #[cfg(all(device_profile, feature = "desktop"))]
    fn the_empty_state_follows_the_appearance() {
        let _guard = crate::style::theme_test_guard();
        crate::widget::census::install_preset_appearances();
        let rect = Rect::new(0, 0, 320, 240);
        let size = crate::core::Size::new(320, 240);

        let centre = |appearance| -> [u8; 4] {
            crate::theme::global_theme_manager().set_appearance(appearance);
            let backdrop = crate::style::theme_manager()
                .current_theme()
                .map(|active| active.colors.background)
                .expect("a preset is active");
            let mut vp = VideoPlayer::new(rect);
            crate::theme::apply_theme_to_widget(&mut vp);
            let frame = render_on(&mut vp, size, backdrop);
            // No source is loaded, so the whole rectangle is the empty-state panel. The sample is
            // near the top-left, clear of the centred "No video loaded" label.
            pixel(&frame, size, 5, 5)
        };

        let dark = centre(crate::theme::AppearanceMode::Dark);
        let light = centre(crate::theme::AppearanceMode::Light);
        assert_ne!(
            dark, light,
            "the empty-state panel must follow the appearance; both were {dark:?}"
        );
    }

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
        vp.set_playback_rate(f32::INFINITY);
        assert_eq!(vp.playback_rate(), 0.1, "a non-finite rate must be ignored");
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
    fn shrinking_duration_clamps_the_clock_and_emits_the_new_time() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_duration(60.0);
        vp.seek(20.0);

        let observed = Arc::new(Mutex::new(None));
        vp.time_updated.connect({
            let observed = Arc::clone(&observed);
            move |time| {
                *observed.lock().unwrap() = Some(*time);
            }
        });

        vp.set_duration(10.0);
        assert_eq!(vp.current_time(), 10.0);
        assert_eq!(*observed.lock().unwrap(), Some(10.0));

        vp.set_duration(f64::INFINITY);
        assert_eq!(vp.duration(), 10.0, "a non-finite duration must be ignored");
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

    /// A negative or non-finite `tick` delta must not move the clock or emit a bad value.
    ///
    /// # What this pins
    ///
    /// `tick` used to add the delta unchecked, so `tick(-10)` on a clock at 5 s gave −5 s and a
    /// `NaN` delta made the clock `NaN` forever — every `current_time >= duration` comparison then
    /// being false, so the control emitted `time_updated` with a `NaN` payload on every tick and
    /// never reported the end. Both are rejected now, leaving the clock exactly where it was.
    #[test]
    fn an_invalid_tick_delta_does_not_corrupt_the_clock() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.load("test.mp4");
        vp.set_duration(60.0);
        vp.play();
        vp.seek(5.0);

        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -10.0] {
            let finished = vp.tick(bad);
            assert!(!finished, "an invalid delta must not report playback finished");
            assert!(vp.current_time().is_finite(), "the clock must stay finite after tick({bad})");
            assert_eq!(vp.current_time(), 5.0, "the clock must not move for tick({bad})");
            assert!(vp.is_playing(), "an invalid delta must not stop playback");
        }

        // A valid delta still advances and stays within `[0, duration]`.
        vp.tick(10.0);
        assert!((vp.current_time() - 15.0).abs() < 1e-9);
    }

    /// A non-finite `seek` is ignored rather than poisoning the clock.
    #[test]
    fn a_non_finite_seek_is_ignored() {
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        vp.set_duration(60.0);
        vp.seek(20.0);
        let before = vp.current_time();
        for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            vp.seek(bad);
            assert_eq!(vp.current_time(), before, "seek({bad}) must be a no-op");
        }
        assert!(vp.progress().is_finite(), "progress must stay finite");
    }

    /// The live setters are reachable through the property channel too.
    ///
    /// `muted`, `playback_rate` and `controls_visible` were one method away on the type but
    /// absent from `get`/`set`/`property_names`/the capability table, so a host driving the
    /// player through the designer route could not mute it or change its rate.
    #[test]
    fn video_player_property_channel_covers_the_live_setters() {
        use crate::widget::capability::WidgetProperties;
        let mut vp = VideoPlayer::new(Rect::new(0, 0, 320, 240));

        vp.set("muted", CapabilityValue::Bool(true)).unwrap();
        assert!(vp.is_muted());
        assert_eq!(vp.get("muted").unwrap(), CapabilityValue::Bool(true));

        vp.set("playback_rate", CapabilityValue::Float(1.5)).unwrap();
        assert!((vp.playback_rate() - 1.5).abs() < 1e-6);

        vp.set("controls_visible", CapabilityValue::Bool(false)).unwrap();
        assert!(!vp.controls_visible());
        vp.set("controls_visible", CapabilityValue::Bool(true)).unwrap();
        assert!(vp.controls_visible());

        for name in ["muted", "playback_rate", "controls_visible"] {
            assert!(vp.property_names().contains(&name), "{name} must be published");
        }
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
