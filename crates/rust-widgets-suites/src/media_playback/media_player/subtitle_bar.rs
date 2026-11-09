// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `SubtitleBar` — a multi-line subtitle band for a video surface (COMP-L04).
//!
//! `VideoPlayer` paints one fixed-height cue band, so a long sentence is truncated. This
//! companion owns a caption band with wrapping, a line cap, a scale factor and its own
//! background opacity, and the host places it over the video surface.
//!
//! Registration: canonical name `subtitle_bar`, kind [`WidgetKind::Label`], registered by
//! `capability::registration` with its own property table (RW-81).

//!
//! # `enabled`
//!
//! `text_changed` is **deliberately not gated** by `enabled`: `set_text` is the host's readout
//! update, and a disabled caption band still displays the cue it was given. The signal reports
//! what the bar shows, not an action.

use crate::media_playback::video_player::{SubtitleCue, VideoPlayer};
use rust_widgets::compat::{Arc, Mutex, Vec};
use rust_widgets::core::{Color, HorizontalAlignment, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::impl_draw_bridge;
use rust_widgets::render::RenderContext;
use rust_widgets::signal::{ConnectionScope, EventSignalRef, GenericSignal};
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::role_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// Where the caption band sits in its host's surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubtitlePosition {
    /// Above the bottom edge (the usual caption placement).
    #[default]
    Bottom,
    /// Below the top edge.
    Top,
}

/// A wrapping, styleable subtitle band.
pub struct SubtitleBar {
    base: BaseWidget,
    text: String,
    max_lines: usize,
    font_scale: f32,
    background_opacity: u8,
    position: SubtitlePosition,
    /// Emitted when the caption text changes.
    pub text_changed: GenericSignal,
    /// The live binding to a player's clock, when one is followed.
    binding: Option<SubtitleBinding>,
}

/// A continuous subscription to a player's clock.
///
/// The bound caption is resolved in the signal callback, so painting never walks the cue list;
/// `_scope` disconnects the subscription when the binding is replaced or dropped (MW-016).
struct SubtitleBinding {
    caption: Arc<Mutex<String>>,
    _scope: ConnectionScope,
}

/// The cue visible at `time`, using the same half-open `[start, end)` rule as the player.
fn cue_at(cues: &[SubtitleCue], time: f64) -> Option<&SubtitleCue> {
    cues.iter().find(|cue| time >= cue.start && time < cue.end)
}

impl SubtitleBar {
    /// Creates an empty caption band.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Label, geometry, "SubtitleBar"),
            text: String::new(),
            max_lines: 3,
            font_scale: 1.0,
            background_opacity: 150,
            position: SubtitlePosition::Bottom,
            text_changed: GenericSignal::new(),
            binding: None,
        }
    }

    /// The explicit caption, used while the band is not following a player.
    ///
    /// While bound, the painted caption is [`Self::current_caption`]; `set_text` still writes this
    /// fallback so stopping the follow returns to it.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the caption. Embedded `\n` starts a new line.
    ///
    /// While the band follows a player, an explicit write takes over: the binding is dropped, so
    /// the new text is what paints and what `get("text")` returns. Writing the fallback while
    /// bound used to change no pixel at all and still emitted `text_changed`.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text && self.binding.is_none() {
            return;
        }
        let before = self.current_caption();
        self.binding = None;
        self.text = text;
        self.base.request_redraw();
        if self.current_caption() != before {
            self.text_changed.emit();
        }
    }

    /// Empties the caption.
    pub fn clear(&mut self) {
        self.set_text(String::new());
    }

    /// The maximum painted line count.
    pub fn max_lines(&self) -> usize {
        self.max_lines
    }

    /// Sets the maximum painted line count, clamped to at least one.
    pub fn set_max_lines(&mut self, max_lines: usize) {
        let max_lines = max_lines.max(1);
        if self.max_lines == max_lines {
            return;
        }
        self.max_lines = max_lines;
        self.base.request_redraw();
    }

    /// The font scale applied to the caption face.
    pub fn font_scale(&self) -> f32 {
        self.font_scale
    }

    /// Sets the font scale, clamped to `0.5..=3.0`.
    pub fn set_font_scale(&mut self, scale: f32) {
        if !scale.is_finite() {
            return;
        }
        let scale = scale.clamp(0.5, 3.0);
        if (self.font_scale - scale).abs() < f32::EPSILON {
            return;
        }
        self.font_scale = scale;
        self.base.request_redraw();
    }

    /// The background opacity, `0..=255`.
    pub fn background_opacity(&self) -> u8 {
        self.background_opacity
    }

    /// Sets the background opacity.
    pub fn set_background_opacity(&mut self, opacity: u8) {
        if self.background_opacity == opacity {
            return;
        }
        self.background_opacity = opacity;
        self.base.request_redraw();
    }

    /// Where the band sits.
    pub fn position(&self) -> SubtitlePosition {
        self.position
    }

    /// Sets where the band sits.
    pub fn set_position(&mut self, position: SubtitlePosition) {
        if self.position == position {
            return;
        }
        self.position = position;
        self.base.request_redraw();
    }

    /// Follows `player` continuously: the band paints the cue at the player's clock.
    ///
    /// The previous binding is disconnected first, so calling this twice does not leave a stale
    /// subscription behind. The cue list is the player's active subtitle track at bind time; a
    /// host that switches tracks calls this again (the player announces the switch through
    /// `subtitle_track_selected`) (MW-016).
    pub fn follow_video_player(&mut self, player: &VideoPlayer) {
        let cues = player
            .subtitle_track()
            .and_then(|index| player.subtitle_tracks().get(index))
            .map(|track| track.cues.clone())
            .unwrap_or_default();
        let initial =
            cue_at(&cues, player.current_time()).map(|cue| cue.text.clone()).unwrap_or_default();
        let caption = Arc::new(Mutex::new(initial));
        let cues = Arc::new(cues);
        let scope = ConnectionScope::new();
        let sink = Arc::clone(&caption);
        let shared_cues = Arc::clone(&cues);
        // A caption change is a repaint the widget owes; the clock's callback has no `&mut self`,
        // so it wakes the host through the base's redraw signal. Without this a standalone band
        // kept painting the cue it was bound at until something else dirtied it. The published
        // `text_changed` fires too: it reports what the bar shows, and a followed cue is exactly
        // that, which is why a bound band used to change its pixels in silence.
        let redraw = self.base.redraw_requested_signal().clone();
        let changed = self.text_changed.clone();
        player.time_updated.connect_ref_scoped(&scope, move |time| {
            let next = cue_at(&shared_cues, *time).map(|cue| cue.text.clone()).unwrap_or_default();
            let mut text = rust_widgets::compat::lock(&sink);
            if *text != next {
                *text = next;
                redraw.emit();
                changed.emit();
            }
        });
        self.binding = Some(SubtitleBinding { caption, _scope: scope });
        self.base.request_redraw();
    }

    /// Whether the band is following a player's clock.
    pub fn is_following_player(&self) -> bool {
        self.binding.is_some()
    }

    /// Stops following the player and returns to the explicit [`Self::text`].
    pub fn stop_following_player(&mut self) {
        if self.binding.take().is_some() {
            self.base.request_redraw();
        }
    }

    /// The caption the band paints: the followed cue when bound, else the explicit text.
    pub fn current_caption(&self) -> String {
        match &self.binding {
            Some(binding) => rust_widgets::compat::lock(&binding.caption).clone(),
            None => self.text.clone(),
        }
    }
}

impl Widget for SubtitleBar {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn capability_name(&self) -> Option<&'static str> {
        Some("subtitle_bar")
    }
    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }
    fn set_state_theme_hook(&mut self) {
        rust_widgets::style::reapply_active_theme_state(self);
    }
    fn size_hint(&self) -> Size {
        Size::new(320, 48)
    }
    fn event_signal_dyn(&self, name: &str) -> Option<EventSignalRef> {
        match name {
            "text_changed" => Some(EventSignalRef::unit("text_changed", &self.text_changed)),
            _ => None,
        }
    }
    impl_draw_bridge!();
    rust_widgets::impl_widget_property_hooks!();
}

/// `SubtitleBar`'s property contract (RW-81).
///
/// Registered under `WidgetKind::Label` with its own canonical name. The text write is the same
/// readout update a host makes from the player clock, so it is deliberately not gated by `enabled`.
impl WidgetProperties for SubtitleBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The readout is the caption the band paints: while bound that is the followed cue, so
            // reporting the explicit fallback here described a string that was not on screen.
            "text" => Ok(CapabilityValue::String(self.current_caption())),
            "max_lines" => Ok(CapabilityValue::UInt(self.max_lines() as u64)),
            "font_scale" => Ok(CapabilityValue::Float(f64::from(self.font_scale()))),
            "background_opacity" => Ok(CapabilityValue::UInt(u64::from(self.background_opacity()))),
            "position" => Ok(CapabilityValue::String(
                match self.position() {
                    SubtitlePosition::Bottom => "bottom",
                    SubtitlePosition::Top => "top",
                }
                .to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "text" => {
                self.set_text(rust_widgets::widget::capability::coercion::expect_string(value)?);
                Ok(())
            }
            "max_lines" => {
                self.set_max_lines(rust_widgets::widget::capability::coercion::expect_usize(
                    value,
                )?);
                Ok(())
            }
            "font_scale" => {
                self.set_font_scale(rust_widgets::widget::capability::coercion::expect_f32(value)?);
                Ok(())
            }
            "background_opacity" => {
                let opacity =
                    rust_widgets::widget::capability::coercion::expect_u8_saturating(value)?;
                self.set_background_opacity(opacity);
                Ok(())
            }
            "position" => {
                let token = rust_widgets::widget::capability::coercion::expect_string(value)?;
                let position = match token.to_ascii_lowercase().as_str() {
                    "bottom" => SubtitlePosition::Bottom,
                    "top" => SubtitlePosition::Top,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_position(position);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        rust_widgets::property_names_of![
            "text",
            "max_lines",
            "font_scale",
            "background_opacity",
            "position",
            BASE_PROPERTY_NAMES
        ]
    }
}

impl EventHandler for SubtitleBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
    }
}

impl Draw for SubtitleBar {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        // Hold the bound caption's guard only for this paint; the signal writes it from the
        // player's clock, so the band stays in step without the host copying text (MW-016).
        let bound =
            self.binding.as_ref().map(|binding| rust_widgets::compat::lock(&binding.caption));
        let text: &str = bound.as_deref().map(String::as_str).unwrap_or(&self.text);
        if rect.width == 0 || rect.height == 0 || text.trim().is_empty() {
            return;
        }
        let style = self.base.style();
        let font = &role_font(style, 14.0 * self.font_scale);
        let line_height = context.measure_text("M", font).height.max(1) + 2;
        let ranges = rust_widgets::widget::text_utils::wrap_to_width(
            text,
            rect.width.saturating_sub(16),
            &mut |text: &str| context.measure_text(text, font).width,
        );
        let lines: Vec<_> = ranges.into_iter().take(self.max_lines).collect();
        let height = (line_height * lines.len() as u32 + 8).min(rect.height.max(line_height));
        let band = match self.position {
            SubtitlePosition::Bottom => {
                Rect::new(rect.x, rect.bottom() - height as i32, rect.width, height)
            }
            SubtitlePosition::Top => Rect::new(rect.x, rect.y, rect.width, height),
        };
        context.fill_rect(band, Color::rgba(0, 0, 0, self.background_opacity));
        for (index, range) in lines.into_iter().enumerate() {
            let Some(line) = text.get(range) else {
                continue;
            };
            context.draw_text_fitted_ellipsis(
                Rect::new(
                    band.x + 8,
                    band.y + 4 + index as i32 * line_height as i32,
                    band.width.saturating_sub(16),
                    line_height,
                ),
                line,
                font,
                Color::WHITE,
                HorizontalAlignment::Center,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// RW-81: the caption, its metrics and its placement all round-trip.
    #[test]
    fn property_contract_round_trips() {
        use rust_widgets::widget::capability::WidgetProperties;
        let mut bar = SubtitleBar::new(Rect::new(0, 0, 320, 48));
        bar.set("text", CapabilityValue::String("hello".into())).expect("text is writable");
        bar.set("max_lines", CapabilityValue::UInt(2)).expect("max_lines is writable");
        bar.set("font_scale", CapabilityValue::Float(1.25)).expect("font_scale is writable");
        bar.set("background_opacity", CapabilityValue::UInt(200)).expect("opacity is writable");
        bar.set("position", CapabilityValue::String("top".into())).expect("position is writable");
        assert_eq!(bar.text(), "hello");
        assert_eq!(bar.max_lines(), 2);
        assert_eq!(bar.position(), SubtitlePosition::Top);
        assert_eq!(bar.get("text"), Ok(CapabilityValue::String("hello".into())));
    }

    #[test]
    fn the_caption_round_trips_and_the_line_cap_has_a_floor() {
        let mut bar = SubtitleBar::new(Rect::new(0, 0, 320, 48));
        bar.set_text("first line\nsecond line");
        assert_eq!(bar.text(), "first line\nsecond line");
        bar.set_max_lines(0);
        assert_eq!(bar.max_lines(), 1);
        bar.clear();
        assert_eq!(bar.text(), "");
    }

    /// The band follows a player's clock through a subscription rather than a one-time snapshot
    /// (MW-016).
    #[test]
    fn following_a_player_keeps_the_caption_in_step_with_the_clock() {
        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(10.0);
        player.set_subtitle_tracks(vec![crate::media_playback::video_player::SubtitleTrack::new(
            "s1",
            "English",
            vec![SubtitleCue::new(0.0, 2.0, "hello"), SubtitleCue::new(2.0, 4.0, "world")],
        )]);
        player.set_subtitle_track(0).expect("the track exists");

        let mut bar = SubtitleBar::new(Rect::new(0, 0, 320, 48));
        bar.set_text("explicit");
        bar.follow_video_player(&player);
        assert!(bar.is_following_player());
        assert_eq!(bar.current_caption(), "hello");

        player.seek(3.0);
        assert_eq!(bar.current_caption(), "world", "the clock updates the caption");
        player.seek(0.5);
        assert_eq!(bar.current_caption(), "hello");

        bar.stop_following_player();
        assert!(!bar.is_following_player());
        assert_eq!(bar.current_caption(), "explicit");
    }

    /// A followed caption change is published, and `text` reports what the band shows.
    ///
    /// The signal is documented as reporting what the bar shows; the bound path repainted the
    /// pixels in silence, and `get("text")` read the explicit fallback rather than the cue.
    #[test]
    fn a_followed_caption_reports_what_it_shows() {
        use rust_widgets::widget::capability::WidgetProperties;

        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(10.0);
        player.set_subtitle_tracks(vec![crate::media_playback::video_player::SubtitleTrack::new(
            "s1",
            "English",
            vec![SubtitleCue::new(0.0, 2.0, "hello"), SubtitleCue::new(2.0, 4.0, "world")],
        )]);
        player.set_subtitle_track(0).expect("the track exists");

        let mut bar = SubtitleBar::new(Rect::new(0, 0, 320, 48));
        bar.set_text("explicit");
        bar.follow_video_player(&player);

        let changes = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&changes);
        bar.text_changed.connect(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        player.seek(3.0);
        assert_eq!(bar.current_caption(), "world");
        assert_eq!(
            bar.get("text"),
            Ok(CapabilityValue::String("world".into())),
            "the readout is the caption on screen"
        );
        assert_eq!(changes.load(std::sync::atomic::Ordering::SeqCst), 1, "the change is published");

        bar.stop_following_player();
        assert_eq!(bar.get("text"), Ok(CapabilityValue::String("explicit".into())));
    }

    /// A followed caption change owes the band a repaint.
    #[test]
    fn a_changed_followed_caption_requests_a_redraw() {
        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(10.0);
        player.set_subtitle_tracks(vec![crate::media_playback::video_player::SubtitleTrack::new(
            "s1",
            "English",
            vec![SubtitleCue::new(0.0, 2.0, "hello"), SubtitleCue::new(2.0, 4.0, "world")],
        )]);
        player.set_subtitle_track(0).expect("the track exists");

        let mut bar = SubtitleBar::new(Rect::new(0, 0, 320, 48));
        bar.follow_video_player(&player);
        let redraws = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let counter = Arc::clone(&redraws);
        bar.base.redraw_requested.connect(move || {
            counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        });

        player.seek(3.0);
        assert_eq!(bar.current_caption(), "world");
        assert_eq!(
            redraws.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the caption changed, so the band must repaint"
        );
        // Seeking inside the same cue changes nothing, so it owes no frame.
        player.seek(3.5);
        assert_eq!(redraws.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    /// Writing the caption through the property takes over from the followed clock.
    ///
    /// While bound, `draw`/`get("text")` read the cue, so the write changed no pixel yet still
    /// emitted `text_changed`; the property did not even round-trip.
    #[test]
    fn an_explicit_write_takes_over_from_the_followed_player() {
        use rust_widgets::widget::capability::WidgetProperties;

        let mut player = VideoPlayer::new(Rect::new(0, 0, 320, 240));
        player.set_duration(10.0);
        player.set_subtitle_tracks(vec![crate::media_playback::video_player::SubtitleTrack::new(
            "s1",
            "English",
            vec![SubtitleCue::new(0.0, 2.0, "hello"), SubtitleCue::new(2.0, 4.0, "world")],
        )]);
        player.set_subtitle_track(0).expect("the track exists");

        let mut bar = SubtitleBar::new(Rect::new(0, 0, 320, 48));
        bar.follow_video_player(&player);
        player.seek(3.0);
        assert_eq!(bar.current_caption(), "world");

        bar.set_text("manual");
        assert!(!bar.is_following_player(), "the write takes over from the clock");
        assert_eq!(bar.get("text"), Ok(CapabilityValue::String("manual".into())));

        // The player is no longer followed, so its clock cannot overwrite the explicit text.
        player.seek(3.5);
        assert_eq!(bar.current_caption(), "manual");
    }
}
