// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MediaScrubber` — a draggable media progress track with a buffered run and chapter marks.
//!
//! The generic sliders in this crate are value controls; a media scrubber needs a buffered
//! second run, chapter ticks and a drag that reports its intermediate positions (COMP-L02).
//!
//! Registration surface (out of scope for this file): canonical name `media_scrubber`, kind
//! [`WidgetKind::Slider`].

use crate::core::{Color, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::metrics::role_font;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};

/// How the handle is drawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScrubberThumbStyle {
    /// A small round dot on the track.
    #[default]
    Dot,
    /// A tall pill handle that is easier to grab.
    Handle,
}

/// A media progress track with buffering and chapter marks.
pub struct MediaScrubber {
    base: BaseWidget,
    value: f64,
    buffered: f64,
    chapters: Vec<f64>,
    step_ms: u64,
    thumb_style: ScrubberThumbStyle,
    dragging: bool,
    /// Emitted while scrubbing with `(active, value)`.
    pub scrubbing: Signal1<(bool, f64)>,
    /// Emitted when a drag or jump commits its final value.
    pub committed: Signal1<f64>,
}

impl MediaScrubber {
    /// Creates a scrubber at `value = 0.0`.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Slider, geometry, "MediaScrubber"),
            value: 0.0,
            buffered: 0.0,
            chapters: Vec::new(),
            step_ms: 0,
            thumb_style: ScrubberThumbStyle::Dot,
            dragging: false,
            scrubbing: Signal1::new(),
            committed: Signal1::new(),
        }
    }

    /// The current value, `0.0..=1.0`.
    pub fn value(&self) -> f64 {
        self.value
    }

    /// Sets the value without emitting; the host owns the media clock.
    pub fn set_value(&mut self, value: f64) {
        let value = if value.is_finite() { value.clamp(0.0, 1.0) } else { return };
        if (self.value - value).abs() < f64::EPSILON {
            return;
        }
        self.value = value;
        self.base.request_redraw();
    }

    /// The buffered fraction, `0.0..=1.0`.
    pub fn buffered(&self) -> f64 {
        self.buffered
    }

    /// Sets the buffered run drawn behind the played run.
    pub fn set_buffered(&mut self, buffered: f64) {
        let buffered = if buffered.is_finite() { buffered.clamp(0.0, 1.0) } else { return };
        if (self.buffered - buffered).abs() < f64::EPSILON {
            return;
        }
        self.buffered = buffered;
        self.base.request_redraw();
    }

    /// Replaces the chapter marks, each a `0.0..=1.0` fraction.
    pub fn set_chapters(&mut self, chapters: Vec<f64>) {
        let chapters: Vec<f64> = chapters
            .into_iter()
            .filter(|value| value.is_finite())
            .map(|v| v.clamp(0.0, 1.0))
            .collect();
        if self.chapters == chapters {
            return;
        }
        self.chapters = chapters;
        self.base.request_redraw();
    }

    /// The chapter marks.
    pub fn chapters(&self) -> &[f64] {
        &self.chapters
    }

    /// The drag snap in milliseconds; `0` disables snapping.
    pub fn step_ms(&self) -> u64 {
        self.step_ms
    }

    /// Sets the drag snap.
    pub fn set_step_ms(&mut self, step_ms: u64) {
        if self.step_ms == step_ms {
            return;
        }
        self.step_ms = step_ms;
    }

    /// The handle style.
    pub fn thumb_style(&self) -> ScrubberThumbStyle {
        self.thumb_style
    }

    /// Sets the handle style.
    pub fn set_thumb_style(&mut self, style: ScrubberThumbStyle) {
        if self.thumb_style == style {
            return;
        }
        self.thumb_style = style;
        self.base.request_redraw();
    }

    /// The track inside the control's rectangle.
    fn track(&self) -> Rect {
        let rect = self.geometry();
        Rect::new(rect.x + 8, rect.y + rect.height as i32 / 2 - 2, rect.width.saturating_sub(16), 4)
    }

    /// Maps an x position to a snapped fraction.
    fn fraction_at(&self, x: i32) -> f64 {
        let track = self.track();
        if track.width == 0 {
            return 0.0;
        }
        let mut value = ((x - track.x) as f64 / track.width as f64).clamp(0.0, 1.0);
        if self.step_ms > 0 {
            // The caller's timeline is unknown here, so snapping is expressed on a 60 s
            // reference; a host that needs exact timeline snapping applies it to the signal.
            let step = (self.step_ms as f64 / 60_000.0).clamp(0.0001, 1.0);
            value = (value / step).round() * step;
            value = value.clamp(0.0, 1.0);
        }
        value
    }

    /// Updates the value and announces an intermediate drag position.
    fn scrub_to(&mut self, value: f64) {
        self.value = value;
        self.scrubbing.emit((true, value));
        self.base.request_redraw();
    }
}

impl Widget for MediaScrubber {
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
        Size::new(240, 24)
    }
    impl_draw_bridge!();
}

impl Draw for MediaScrubber {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let ink = style.text_color.unwrap_or(Color::rgba(90, 90, 90, 255));
        let accent = style.background_color.unwrap_or(Color::rgba(70, 140, 230, 255));
        let track = self.track();
        context.fill_rounded_rect(track, 2, ink.with_alpha(50));
        if self.buffered > 0.0 {
            let width = (track.width as f64 * self.buffered) as u32;
            context.fill_rounded_rect(
                Rect::new(track.x, track.y, width, track.height),
                2,
                ink.with_alpha(90),
            );
        }
        let played = (track.width as f64 * self.value) as u32;
        if played > 0 {
            context.fill_rounded_rect(Rect::new(track.x, track.y, played, track.height), 2, accent);
        }
        for chapter in &self.chapters {
            let x = track.x + (track.width as f64 * chapter) as i32;
            context.draw_line(
                Point::new(x, track.y - 4),
                Point::new(x, track.bottom() + 4),
                ink.with_alpha(140),
            );
        }
        let handle_x = track.x + (track.width as f64 * self.value).round() as i32;
        let handle_y = track.y + track.height as i32 / 2;
        match self.thumb_style {
            ScrubberThumbStyle::Dot => {
                context.fill_circle(Point::new(handle_x, handle_y), 6, accent);
            }
            ScrubberThumbStyle::Handle => {
                context.fill_rounded_rect(
                    Rect::new(handle_x - 3, rect.y + 2, 6, rect.height.saturating_sub(4)),
                    3,
                    accent,
                );
            }
        }
        // A value readout makes the current fraction legible while dragging.
        let label = format!("{:.0}%", self.value * 100.0);
        let font = &role_font(style, 10.0);
        context.draw_text(
            Point::new(rect.x + 4, rect.y + 1),
            &label,
            font,
            ink.with_alpha(180),
            HorizontalAlignment::Left,
        );
    }
}

impl EventHandler for MediaScrubber {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button: 1, .. } => {
                if self.geometry().contains_point(*pos) {
                    self.dragging = true;
                    let value = self.fraction_at(pos.x);
                    self.scrub_to(value);
                }
            }
            Event::Drag { pos, .. } if self.dragging => {
                let value = self.fraction_at(pos.x);
                self.scrub_to(value);
            }
            Event::MouseRelease { button: 1, .. } if self.dragging => {
                self.dragging = false;
                self.scrubbing.emit((false, self.value));
                self.committed.emit(self.value);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_drag_reports_intermediate_values_and_commits_once() {
        let mut scrubber = MediaScrubber::new(Rect::new(0, 0, 200, 24));
        let values = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let committed = std::sync::Arc::new(std::sync::Mutex::new(0usize));
        let v = values.clone();
        scrubber.scrubbing.connect(move |value: std::sync::Arc<(bool, f64)>| {
            v.lock().unwrap().push(*value);
        });
        let c = committed.clone();
        scrubber.committed.connect(move |_| {
            *c.lock().unwrap() += 1;
        });
        scrubber.handle_event(&Event::MousePress {
            pos: Point::new(8, 12),
            button: 1,
            modifiers: 0,
        });
        scrubber.handle_event(&Event::Drag {
            pos: Point::new(108, 12),
            touch_id: 0,
            delta: Point::new(100, 0),
        });
        scrubber.handle_event(&Event::MouseRelease { pos: Point::new(108, 12), button: 1 });
        let seen = values.lock().unwrap();
        assert!(seen.len() >= 2, "a drag reports more than the press");
        assert!(seen.last().expect("a sample").1 > 0.4);
        assert_eq!(*committed.lock().unwrap(), 1);
    }
}
