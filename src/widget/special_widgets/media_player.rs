// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! MediaPlayer widget.

#[cfg(test)]
use crate::core::Point;
use crate::core::{Color, Font, HorizontalAlignment, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_u32, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::dimensions;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The inset a media player's own chrome keeps from its surface edges: 10.
///
/// One value for the two stacked labels and the transport rule, so all three share a leading
/// edge and a bottom margin instead of each spelling its own `10`/`6`/`8` offsets.
const MEDIA_PLAYER_PADDING: i32 = 10;

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
    impl_draw_bridge!();
    impl_widget_property_hooks!();
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
                    if self.duration_ms > 0 && bar_rect.width > 0 {
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

        context.fill_rect(rect, surface);
        context.draw_rect(rect, divider);

        let title = self
            .source
            .as_deref()
            .map(|src| src.rsplit('/').next().unwrap_or(src))
            .unwrap_or("No media");
        let state = if self.playing { "Playing" } else { "Paused" };
        let vol = if self.muted { "Muted".to_string() } else { format!("Vol {}", self.volume) };
        let fs = if self.fullscreen { "Fullscreen" } else { "Window" };

        // Both lines are fitted to the control's width. The title comes from a file name,
        // which has no length limit, and the status line is three joined words; neither was
        // bounded, so a long file name simply kept going past the player's right edge. Both
        // share the one padding constant with the transport rule below them.
        let font = Font::default();
        context.draw_text_fitted(
            Rect::new(
                rect.x + MEDIA_PLAYER_PADDING,
                rect.y + MEDIA_PLAYER_PADDING / 2,
                rect.width.saturating_sub((MEDIA_PLAYER_PADDING * 2) as u32),
                16,
            ),
            title,
            &font,
            ink,
            HorizontalAlignment::Left,
        );
        context.draw_text_fitted(
            Rect::new(
                rect.x + MEDIA_PLAYER_PADDING,
                rect.y + MEDIA_PLAYER_PADDING / 2 + 18,
                rect.width.saturating_sub((MEDIA_PLAYER_PADDING * 2) as u32),
                16,
            ),
            &format!("{state} | {vol} | {fs}"),
            &font,
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
        let _guard = crate::theme::theme_test_guard();
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
        player.handle_event(&Event::MousePress { pos: Point::new(5, 62), button: 1 });
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
