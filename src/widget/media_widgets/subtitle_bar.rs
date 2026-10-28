// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `SubtitleBar` — a multi-line subtitle band for a video surface (COMP-L04).
//!
//! `VideoPlayer` paints one fixed-height cue band, so a long sentence is truncated. This
//! companion owns a caption band with wrapping, a line cap, a scale factor and its own
//! background opacity, and the host places it over the video surface.
//!
//! Registration surface (out of scope for this file): canonical name `subtitle_bar`, kind
//! [`WidgetKind::Label`].

//!
//! # `enabled`
//!
//! `text_changed` is **deliberately not gated** by `enabled`: `set_text` is the host's readout
//! update, and a disabled caption band still displays the cue it was given. The signal reports
//! what the bar shows, not an action.

use crate::compat::{Arc, Mutex, Vec};
use crate::core::{Color, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::{ConnectionScope, GenericSignal};
use crate::widget::media_widgets::video_player::{SubtitleCue, VideoPlayer};
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

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

    /// The current caption.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Replaces the caption. Embedded `\n` starts a new line.
    pub fn set_text(&mut self, text: impl Into<String>) {
        let text = text.into();
        if self.text == text {
            return;
        }
        self.text = text;
        self.base.request_redraw();
        self.text_changed.emit();
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
        player.time_updated.connect_ref_scoped(&scope, move |time| {
            let next = cue_at(&shared_cues, *time).map(|cue| cue.text.clone()).unwrap_or_default();
            let mut text = crate::compat::lock(&sink);
            if *text != next {
                *text = next;
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
            Some(binding) => crate::compat::lock(&binding.caption).clone(),
            None => self.text.clone(),
        }
    }
}

impl Widget for SubtitleBar {
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
        Size::new(320, 48)
    }
    impl_draw_bridge!();
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
        let bound = self.binding.as_ref().map(|binding| crate::compat::lock(&binding.caption));
        let text: &str = bound.as_deref().map(String::as_str).unwrap_or(&self.text);
        if rect.width == 0 || rect.height == 0 || text.trim().is_empty() {
            return;
        }
        let style = self.base.style();
        let font = &role_font(style, 14.0 * self.font_scale);
        let line_height = context.measure_text("M", font).height.max(1) + 2;
        let ranges = crate::widget::text_utils::wrap_to_width(
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
            context.draw_text_fitted_cow(
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
        player.set_subtitle_tracks(vec![
            crate::widget::media_widgets::video_player::SubtitleTrack::new(
                "s1",
                "English",
                vec![SubtitleCue::new(0.0, 2.0, "hello"), SubtitleCue::new(2.0, 4.0, "world")],
            ),
        ]);
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
}
