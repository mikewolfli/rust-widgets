// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MediaScrubber` — a draggable media progress track with a buffered run and chapter marks.
//!
//! The generic sliders in this crate are value controls; a media scrubber needs a buffered
//! second run, chapter ticks and a drag that reports its intermediate positions (COMP-L02).
//!
//! Registration: canonical name `media_scrubber`, kind [`WidgetKind::Slider`], registered by
//! `capability::registration` with its own property table (RW-81).

use core::fmt::Write as _;

use rust_widgets::compat::String;
use rust_widgets::core::{Color, HorizontalAlignment, Point, Rect, Size};
use rust_widgets::event::{Event, EventHandler};
use rust_widgets::render::RenderContext;
use rust_widgets::signal::Signal1;
use rust_widgets::widget::capability::properties_trait::{base_property_get, base_property_set};
use rust_widgets::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use rust_widgets::widget::capability::WidgetProperties;
use rust_widgets::widget::metrics::role_font;
use rust_widgets::widget::{BaseWidget, Draw, Widget, WidgetKind};
use rust_widgets::{impl_widget_property_hooks, property_names_of};
use rust_widgets::impl_draw_bridge;

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
    /// The percentage readout, reused between frames so a repaint does not build a `String`.
    readout_text: String,
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
            readout_text: String::new(),
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
        rust_widgets::style::reapply_active_theme_state(self);
    }
    impl_widget_property_hooks!();
    fn size_hint(&self) -> Size {
        Size::new(240, 24)
    }
    impl_draw_bridge!();
}

/// `MediaScrubber`'s property contract (RW-81).
///
/// Registered under `WidgetKind::Slider` with its own canonical name. The value and the buffered
/// run are independent reads, not one progress pair, because a host updates them at different rates.
impl WidgetProperties for MediaScrubber {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "value" => Ok(CapabilityValue::Float(self.value())),
            "buffered" => Ok(CapabilityValue::Float(self.buffered())),
            "chapter_count" => Ok(CapabilityValue::UInt(self.chapters().len() as u64)),
            "thumb_style" => Ok(CapabilityValue::String(
                match self.thumb_style() {
                    ScrubberThumbStyle::Dot => "dot",
                    ScrubberThumbStyle::Handle => "handle",
                }
                .to_string(),
            )),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "value" => {
                let value = rust_widgets::widget::capability::coercion::expect_f64(value)?;
                self.set_value(value);
                Ok(())
            }
            "buffered" => {
                let buffered = rust_widgets::widget::capability::coercion::expect_f64(value)?;
                self.set_buffered(buffered);
                Ok(())
            }
            "thumb_style" => {
                let token = rust_widgets::widget::capability::coercion::expect_string(value)?;
                let style = match token.to_ascii_lowercase().as_str() {
                    "dot" => ScrubberThumbStyle::Dot,
                    "handle" => ScrubberThumbStyle::Handle,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_thumb_style(style);
                Ok(())
            }
            "chapter_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["value", "buffered", "chapter_count", "thumb_style", BASE_PROPERTY_NAMES]
    }
}

impl Draw for MediaScrubber {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        let style = self.base.style();
        let (ink, accent) = rust_widgets::style::palette_shared()
            .map_or((Color::rgba(90, 90, 90, 255), Color::rgba(70, 140, 230, 255)), |palette| {
                (palette.foreground, palette.primary)
            });
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
        self.readout_text.clear();
        let _ = write!(self.readout_text, "{:.0}%", self.value * 100.0);
        let font = &role_font(style, 10.0);
        context.draw_text(
            Point::new(rect.x + 4, rect.y + 1),
            &self.readout_text,
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

    /// RW-81: value, buffered run and thumb style are all part of the property contract.
    #[test]
    fn property_contract_round_trips() {
        use rust_widgets::widget::capability::WidgetProperties;
        let mut scrubber = MediaScrubber::new(Rect::new(0, 0, 200, 24));
        scrubber.set("value", CapabilityValue::Float(0.5)).expect("value is writable");
        scrubber.set("buffered", CapabilityValue::Float(0.75)).expect("buffered is writable");
        scrubber
            .set("thumb_style", CapabilityValue::String("handle".into()))
            .expect("thumb_style is writable");
        assert_eq!(scrubber.value(), 0.5);
        assert_eq!(scrubber.buffered(), 0.75);
        assert_eq!(scrubber.thumb_style(), ScrubberThumbStyle::Handle);
        assert_eq!(scrubber.get("value"), Ok(CapabilityValue::Float(0.5)));
        assert_eq!(
            scrubber.set("chapter_count", CapabilityValue::UInt(0)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

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
