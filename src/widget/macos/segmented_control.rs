// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! `MacSegmentedControl` — a multi-way picker on a unified desktop track.
//!
//! A desktop segmented picker is not the touch platform's pill switcher: it is one narrow
//! unified track whose segments share a single rounded outline, with the active segment filled
//! by the accent colour. This control draws exactly that shape and keeps the interaction model
//! the rest of the crate uses — click, arrow keys, Home/End — while publishing the selection
//! through the capability table.

use crate::compat::Cow;
use crate::core::{HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{profile_metrics, role_font, ControlMetricsProfile};
use crate::widget::text_utils::elide_to_width;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

use super::{
    centred_band, draw_focus_ring, palette, MAC_SEGMENTED_CONTROL_HEIGHT,
    MAC_SEGMENTED_DEFAULT_WIDTH, MAC_SEGMENT_LABEL_INSET,
};

/// The vertical inset of the selected segment's fill from the track, in points.
const SELECTED_SEGMENT_INSET: i32 = 2;

/// A desktop segmented picker.
///
/// The control owns an ordered list of captions and one selected index. Clicking a segment, or
/// pressing an arrow key while the control has focus, selects it and emits
/// [`Self::value_changed`] with the new index. An empty control still paints its track, so a
/// freshly constructed picker is visible before the host supplies segments.
pub struct MacSegmentedControl {
    base: BaseWidget,
    segments: Vec<String>,
    selected_index: usize,
    hovered_index: Option<usize>,
    /// Emitted when the selected segment changes, with the new index.
    pub value_changed: Signal1<usize>,
}

impl MacSegmentedControl {
    /// Creates an empty picker with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SegmentedControl, geometry, "MacSegmentedControl"),
            segments: Vec::new(),
            selected_index: 0,
            hovered_index: None,
            value_changed: Signal1::new(),
        }
    }

    /// Replaces the segment captions, keeping the selection in range.
    ///
    /// A rebuilt caption list is a reset, not a selection change, so no `value_changed` is
    /// emitted: the host asked for new segments, it did not pick one.
    pub fn set_segments(&mut self, segments: Vec<String>) {
        if self.segments == segments {
            return;
        }
        self.segments = segments;
        if self.segments.is_empty() {
            self.selected_index = 0;
            self.hovered_index = None;
        } else {
            self.selected_index = self.selected_index.min(self.segments.len() - 1);
            self.hovered_index = self.hovered_index.filter(|index| *index < self.segments.len());
        }
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The segment captions, in display order.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// The number of segments.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// The selected index, clamped to the segment list (`0` when the list is empty).
    pub fn selected_index(&self) -> usize {
        if self.segments.is_empty() {
            0
        } else {
            self.selected_index.min(self.segments.len() - 1)
        }
    }

    /// The caption of the selected segment, or `None` when the list is empty.
    pub fn selected_label(&self) -> Option<&str> {
        self.segments.get(self.selected_index()).map(String::as_str)
    }

    /// Selects `index`, clamped to the list. Returns whether the selection changed.
    ///
    /// Emits [`Self::value_changed`] only on a real change, so a host that writes back the
    /// value it just read does not see a spurious signal.
    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if self.segments.is_empty() {
            return false;
        }
        let clamped = index.min(self.segments.len() - 1);
        if clamped == self.selected_index {
            return false;
        }
        self.selected_index = clamped;
        self.base.request_redraw();
        self.value_changed.emit(clamped);
        true
    }

    /// Moves the selection by `delta` segments, clamped at both ends.
    fn move_selection(&mut self, delta: isize) {
        if self.segments.is_empty() {
            return;
        }
        let last = self.segments.len() as isize - 1;
        let next = (self.selected_index() as isize + delta).clamp(0, last) as usize;
        self.set_selected_index(next);
    }

    /// The track the control paints: full width, one segment row tall, centred.
    fn track_band(&self) -> Rect {
        centred_band(self.geometry(), MAC_SEGMENTED_CONTROL_HEIGHT)
    }

    /// The corner radius of the track, taken from the desktop profile's button radius.
    fn track_radius(&self) -> u32 {
        let radius = profile_metrics(ControlMetricsProfile::Macos).button_radius;
        radius.min(self.track_band().height / 2)
    }

    /// The rectangle of segment `index`, or `None` when it is not in the list.
    ///
    /// The last segment absorbs the division remainder, so the segments always tile the track
    /// exactly and the selection fill cannot leave a one-pixel gap at the trailing edge.
    fn segment_rect(&self, index: usize) -> Option<Rect> {
        let band = self.track_band();
        let count = self.segments.len();
        if count == 0 || index >= count {
            return None;
        }
        let width = band.width as i32 / count as i32;
        let x = band.x + index as i32 * width;
        let width =
            if index + 1 == count { (band.right() - x).max(0) as u32 } else { width.max(0) as u32 };
        Some(Rect::new(x, band.y, width, band.height))
    }

    /// The segment under `pos`, if any.
    fn segment_at(&self, pos: Point) -> Option<usize> {
        if !self.track_band().contains_point(pos) {
            return None;
        }
        (0..self.segments.len())
            .find(|index| self.segment_rect(*index).is_some_and(|rect| rect.contains_point(pos)))
    }
}

impl Widget for MacSegmentedControl {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        Size::new(MAC_SEGMENTED_DEFAULT_WIDTH, MAC_SEGMENTED_CONTROL_HEIGHT)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::SegmentedControl
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();

    /// Resolves a published event name to the control's own signal.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "value_changed" => {
                Some(EventSignalRef::mapped("value_changed", &self.value_changed, |value| {
                    CapabilityValue::UInt(*value as u64)
                }))
            }
            _ => None,
        }
    }
}

impl WidgetProperties for MacSegmentedControl {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_index" => Ok(CapabilityValue::UInt(self.selected_index() as u64)),
            "segment_count" => Ok(CapabilityValue::UInt(self.segment_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => {
                self.set_selected_index(expect_usize(value)?);
                Ok(())
            }
            // Derived from the segment list, which owns it.
            "segment_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["selected_index", "segment_count", BASE_PROPERTY_NAMES]
    }
}

impl Draw for MacSegmentedControl {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let band = self.track_band();
        let radius = self.track_radius();
        let colors = palette();
        let style = self.base.style();
        let font = role_font(style, 11.0);
        let count = self.segments.len();

        // The unified track. Painted even with no segments, so a freshly constructed control
        // occupies the space it claims instead of rendering as a hole in the layout.
        context.fill_rounded_rect(band, radius, colors.surface);
        context.draw_rounded_rect_stroke(band, radius, colors.separator, 1);

        // Dividers between adjacent segments that are both unselected. A divider next to the
        // selection would cut into the accent fill, which is why it is skipped there.
        if count > 1 {
            for index in 1..count {
                if index == self.selected_index() || index - 1 == self.selected_index() {
                    continue;
                }
                let Some(segment) = self.segment_rect(index) else {
                    continue;
                };
                context.draw_line(
                    Point::new(segment.x, segment.y + SELECTED_SEGMENT_INSET),
                    Point::new(segment.x, segment.bottom() - SELECTED_SEGMENT_INSET),
                    colors.separator,
                );
            }
        }

        // The selection fill, inset so it reads as a thumb inside the track rather than as a
        // second rectangle drawn over it.
        if let Some(segment) = self.segment_rect(self.selected_index()) {
            let inset = SELECTED_SEGMENT_INSET.min(segment.height as i32 / 2);
            let width = segment.width.saturating_sub((2 * inset) as u32);
            let height = segment.height.saturating_sub((2 * inset) as u32);
            if width > 0 && height > 0 {
                let thumb = Rect::new(segment.x + inset, segment.y + inset, width, height);
                let thumb_radius = radius.saturating_sub(inset as u32);
                context.fill_rounded_rect(thumb, thumb_radius, colors.accent);
            }
        }

        // Labels last, so the selection fill cannot cover the text it belongs to.
        for index in 0..count {
            let Some(segment) = self.segment_rect(index) else {
                continue;
            };
            let Some(label) = self.segments.get(index) else {
                continue;
            };
            let inset = MAC_SEGMENT_LABEL_INSET;
            let available = segment.width.saturating_sub((2 * inset) as u32);
            if available == 0 {
                continue;
            }
            let mut measure = |text: &str| context.measure_text(text, &font).width;
            let shown: Cow<'_, str> = elide_to_width(label, available, &mut measure);
            let ink = if index == self.selected_index() {
                colors.accent.contrast_color()
            } else if self.hovered_index == Some(index) {
                colors.ink
            } else {
                colors.ink.blend(&colors.muted, 0.25)
            };
            let line = context.text_line(segment, &font);
            context.draw_text(
                Point::new(segment.center().x, line.y),
                shown.as_ref(),
                &font,
                ink,
                HorizontalAlignment::Center,
            );
        }

        draw_focus_ring(context, band, radius, &self.base, colors.accent);
    }
}

impl EventHandler for MacSegmentedControl {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                let hovered = self.segment_at(*pos);
                if hovered != self.hovered_index {
                    self.hovered_index = hovered;
                    self.base.request_redraw();
                }
            }
            Event::MouseLeave { .. } => {
                if self.hovered_index.take().is_some() {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.segment_at(*pos) {
                    self.set_selected_index(index);
                }
            }
            Event::KeyPress { key, .. } => match crate::shortcut::Key::from_key_code(*key) {
                Some(crate::shortcut::Key::Left) | Some(crate::shortcut::Key::Up) => {
                    self.move_selection(-1)
                }
                Some(crate::shortcut::Key::Right) | Some(crate::shortcut::Key::Down) => {
                    self.move_selection(1)
                }
                Some(crate::shortcut::Key::Home) => {
                    self.set_selected_index(0);
                }
                Some(crate::shortcut::Key::End) if !self.segments.is_empty() => {
                    self.set_selected_index(self.segments.len() - 1);
                }
                _ => {}
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::test_support::Recorder;
    use super::*;
    use crate::core::Color;
    use crate::render::RenderCommand;

    fn sample() -> MacSegmentedControl {
        let mut control = MacSegmentedControl::new(Rect::new(0, 0, 240, 24));
        control.set_segments(vec!["Day".into(), "Week".into(), "Month".into()]);
        control
    }

    #[test]
    fn a_fresh_picker_has_no_segments_and_selects_nothing() {
        let control = MacSegmentedControl::new(Rect::new(0, 0, 240, 24));
        assert_eq!(control.segment_count(), 0);
        assert_eq!(control.selected_index(), 0);
        assert!(control.selected_label().is_none());
        // The desktop metrics, not the touch ones: one 24-point row.
        assert_eq!(control.size_hint(), Size::new(MAC_SEGMENTED_DEFAULT_WIDTH, 24));
    }

    #[test]
    fn setting_segments_keeps_the_selection_in_range() {
        let mut control = sample();
        assert_eq!(control.segment_count(), 3);
        assert_eq!(control.selected_index(), 0);

        assert!(control.set_selected_index(2));
        assert_eq!(control.selected_label(), Some("Month"));
        // Clamped rather than refused: a host rebuilding its list should not have to pre-clamp.
        control.set_selected_index(99);
        assert_eq!(control.selected_index(), 2);
    }

    #[test]
    fn a_selection_change_emits_and_a_repeat_does_not() {
        use std::sync::{Arc, Mutex};

        let mut control = sample();
        let seen = Arc::new(Mutex::new(usize::MAX));
        control.value_changed.connect({
            let seen = Arc::clone(&seen);
            move |index: Arc<usize>| {
                *seen.lock().unwrap() = *index;
            }
        });

        assert!(control.set_selected_index(1));
        assert_eq!(*seen.lock().unwrap(), 1);

        // Writing the value it already reports must not fire the signal again.
        assert!(!control.set_selected_index(1));
        assert_eq!(*seen.lock().unwrap(), 1);
    }

    #[test]
    fn a_click_selects_the_segment_under_the_pointer() {
        let mut control = sample();
        // Segment 2 of 3 spans x in [160, 240) on a 240-wide track.
        control.handle_event(&Event::MousePress {
            pos: Point::new(200, 12),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(control.selected_index(), 2);
    }

    #[test]
    fn arrow_keys_move_the_selection_and_stop_at_the_ends() {
        let mut control = sample();
        control.handle_event(&Event::KeyPress { key: crate::event::key_codes::LEFT, modifiers: 0 });
        assert_eq!(control.selected_index(), 0, "the first segment is a floor");
        control
            .handle_event(&Event::KeyPress { key: crate::event::key_codes::RIGHT, modifiers: 0 });
        assert_eq!(control.selected_index(), 1);
    }

    #[test]
    fn a_disabled_picker_ignores_clicks() {
        let mut control = sample();
        control.set_enabled(false);
        control.handle_event(&Event::MousePress {
            pos: Point::new(200, 12),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(control.selected_index(), 0);
    }

    #[test]
    fn the_property_route_reads_and_writes_the_selection() {
        let mut control = sample();
        assert_eq!(control.get("selected_index"), Ok(CapabilityValue::UInt(0)));
        control.set("selected_index", CapabilityValue::UInt(2)).unwrap();
        assert_eq!(control.get("selected_index"), Ok(CapabilityValue::UInt(2)));
        assert_eq!(control.get("segment_count"), Ok(CapabilityValue::UInt(3)));
        assert_eq!(
            control.set("segment_count", CapabilityValue::UInt(9)),
            Err(CapabilityAccessError::ReadOnlyProperty)
        );
    }

    #[test]
    fn the_picker_paints_a_track_even_with_no_segments() {
        let mut control = MacSegmentedControl::new(Rect::new(0, 0, 240, 24));
        let mut recorder = Recorder::new(240, 24);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            control.draw(&mut context);
        }
        recorder.end_frame();
        assert!(
            recorder
                .commands
                .iter()
                .any(|command| matches!(command, RenderCommand::FillRoundedRect { .. })),
            "an empty picker still paints its track"
        );
        assert!(
            !recorder
                .commands
                .iter()
                .any(|command| matches!(command, RenderCommand::DrawText { .. })),
            "with no segments there is no caption to paint"
        );
    }

    #[test]
    fn the_selected_segment_is_filled_and_its_label_painted_after_it() {
        let mut control = sample();
        let mut recorder = Recorder::new(240, 24);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            control.draw(&mut context);
        }
        recorder.end_frame();

        let fill_at = recorder.commands.iter().position(|command| {
            matches!(command, RenderCommand::FillRoundedRect { radius: 4, .. })
        });
        let label_at = recorder.commands.iter().position(|command| {
            matches!(command, RenderCommand::DrawText { text, .. } if text.as_ref() == "Day")
        });
        assert!(fill_at.is_some(), "the selected segment is filled by the accent");
        assert!(label_at.is_some(), "every segment paints its caption");
        assert!(
            fill_at.unwrap() < label_at.unwrap(),
            "the caption is painted after the selection fill, so it stays legible"
        );
    }

    #[test]
    fn every_painted_label_stays_inside_the_track() {
        let mut control = sample();
        let mut recorder = Recorder::new(240, 24);
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            control.draw(&mut context);
        }
        recorder.end_frame();
        for command in &recorder.commands {
            match command {
                RenderCommand::DrawText { origin, .. } => {
                    assert!(origin.y >= 0 && origin.y < 24, "text stays in its row");
                }
                RenderCommand::FillRoundedRect { rect, .. } => {
                    assert!(rect.x >= 0 && rect.right() <= 240);
                    assert!(rect.y >= 0 && rect.bottom() <= 24);
                }
                _ => {}
            }
        }
    }
}
