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

use crate::compat::Vec;
use crate::core::{Color, HorizontalAlignment, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::GenericSignal;
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
        if rect.width == 0 || rect.height == 0 || self.text.trim().is_empty() {
            return;
        }
        let style = self.base.style();
        let font = &role_font(style, 14.0 * self.font_scale);
        let line_height = context.measure_text("M", font).height.max(1) + 2;
        let ranges = crate::widget::text_utils::wrap_to_width(
            &self.text,
            rect.width.saturating_sub(16),
            &mut |text: &str| context.measure_text(text, font).width,
        );
        let lines: Vec<_> = ranges.into_iter().take(self.max_lines).collect();
        let height =
            (line_height * lines.len() as u32 + 8).min(rect.height.max(line_height));
        let band = match self.position {
            SubtitlePosition::Bottom => {
                Rect::new(rect.x, rect.bottom() - height as i32, rect.width, height)
            }
            SubtitlePosition::Top => Rect::new(rect.x, rect.y, rect.width, height),
        };
        context.fill_rect(band, Color::rgba(0, 0, 0, self.background_opacity));
        for (index, range) in lines.into_iter().enumerate() {
            let Some(text) = self.text.get(range) else {
                continue;
            };
            context.draw_text_fitted_cow(
                Rect::new(
                    band.x + 8,
                    band.y + 4 + index as i32 * line_height as i32,
                    band.width.saturating_sub(16),
                    line_height,
                ),
                text,
                font,
                Color::WHITE,
                HorizontalAlignment::Center,
            );
        }
    }
}
