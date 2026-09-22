// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! CupertinoSegmentedControl — iOS-style segmented control.
//!
//! A pill-shaped container with horizontally arranged segments. The selected
//! segment has a sliding highlight. Clicking a segment selects it and emits
//! a `value_changed` signal with the segment index.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_usize;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// iOS-style segmented control.
///
/// Displays a pill-shaped container with horizontally arranged text segments.
/// The selected segment is highlighted with a sliding white indicator.
/// Emits `value_changed` when the user clicks a segment.
pub struct CupertinoSegmentedControl {
    base: BaseWidget,
    segments: Vec<String>,
    selected_index: usize,
    /// Emitted when the selected segment changes with the new index.
    pub value_changed: Signal1<usize>,
}

impl CupertinoSegmentedControl {
    /// Creates a new CupertinoSegmentedControl with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        let base = BaseWidget::new(
            WidgetKind::CupertinoSegmentedControl,
            geometry,
            "CupertinoSegmentedControl",
        );
        Self { base, segments: Vec::new(), selected_index: 0, value_changed: Signal1::new() }
    }

    /// Sets the segment labels, replacing all existing segments.
    /// Resets selection to 0 if the new list is non-empty.
    pub fn set_segments(&mut self, segments: Vec<String>) {
        self.segments = segments;
        if self.selected_index >= self.segments.len().max(1) {
            self.selected_index = if self.segments.is_empty() {
                0
            } else {
                self.selected_index.min(self.segments.len() - 1)
            };
        }
        self.base.request_redraw();
    }

    /// Returns a reference to the segment labels.
    pub fn segments(&self) -> &[String] {
        &self.segments
    }

    /// Sets the selected segment index. Clamped to valid range.
    pub fn set_selected_index(&mut self, index: usize) {
        if self.segments.is_empty() {
            self.selected_index = 0;
            return;
        }
        let clamped = index.min(self.segments.len() - 1);
        if clamped != self.selected_index {
            self.selected_index = clamped;
            self.value_changed.emit(clamped);
            self.base.request_redraw();
        }
    }

    /// Returns the currently selected index.
    pub fn selected_index(&self) -> usize {
        if self.segments.is_empty() {
            0
        } else {
            self.selected_index.min(self.segments.len() - 1)
        }
    }

    /// Returns the number of segments.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// The track the control actually paints: full width,
    /// `dimensions::SEGMENTED_CONTROL_HEIGHT` tall, centred in the rectangle it was given.
    ///
    /// # Why the track is not the rectangle
    ///
    /// A segmented control's chrome is one row of segments. Taking `rect.height` made a
    /// 240x120 census cell a **240x120 stadium**, and it disagreed with the 32 px `size_hint`
    /// the control reports. The band is the single derivation the paint and the hit test share.
    fn track_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::SEGMENTED_CONTROL_HEIGHT)
    }
}

impl Widget for CupertinoSegmentedControl {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> Size {
        crate::core::Size::new(300, 32)
    }

    fn kind(&self) -> WidgetKind {
        WidgetKind::CupertinoSegmentedControl
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `CupertinoSegmentedControl`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_input.in.rs` / `access_write_input.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `selected_index` reads the
/// control's real selection; `segment_count` is derived from the segment list and
/// has no setter, so writes are refused with
/// [`CapabilityAccessError::ReadOnlyProperty`].
impl WidgetProperties for CupertinoSegmentedControl {
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

impl Draw for CupertinoSegmentedControl {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        let seg_count = self.segments.len();
        // ── The track actually painted ──
        //
        // `rect` is the area the control was *given*; the control's own chrome is one row of
        // segments, `dimensions::SEGMENTED_CONTROL_HEIGHT` tall and centred. Painting the
        // track across the whole rectangle made a 240x120 census cell a bare 240x120 stadium
        // with no segment division in it — the defect this replaces — and it disagreed with the
        // 32 px size the control reports. A control with no segments still paints its track,
        // which is what keeps a freshly constructed control visible; that track is now the
        // fixed-height pill rather than the whole canvas.
        let track_rect = self.track_band();
        let seg_w = if seg_count == 0 {
            track_rect.width as i32
        } else {
            track_rect.width as i32 / seg_count as i32
        };
        let corner_radius = track_rect.height / 2;

        // Chrome colours resolve explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be
        // a literal, so a light/dark switch left the pill, its indicator and its labels
        // unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's
        // mutex is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("cupertino_segmented_control");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme. The control is not in the role table, so it
        // classifies as `Surface` and its resolved background is the window fill itself; the
        // track below therefore derives its own distinct surface rather than painting the
        // window's. The selected segment is the control's value indicator, so its sliding
        // highlight reads the theme's primary.
        let (window_fill, foreground, primary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => {
                    (active.colors.background, active.colors.foreground, active.colors.primary)
                }
                None => (Color::rgb(240, 240, 240), Color::BLACK, Color::rgb(0, 122, 255)),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The track: one step from the window fill toward the text colour, so the pill is a
        // distinct element on a light theme and on a dark one. The filter is on the
        // **resolved** value, not only on the theme's: the active theme is applied to every
        // control before it is drawn, so `style.background_color` already holds `Surface`'s
        // window fill and letting it through unfiltered is exactly the invisible-pill defect
        // this guards against. A caller's own colour still wins.
        let track_from_theme = window_fill.blend(&ink, 0.08);
        let track_surface = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => track_from_theme,
        };
        // The slider's knob makes the same distinction between the track it sits on and the
        // page behind it: raised by default, recessed while disabled.
        let track_color = if self.base.is_enabled() {
            track_surface
        } else {
            track_surface.blend(&window_fill, 0.50)
        };
        context.fill_rounded_rect(track_rect, corner_radius, track_color);

        if seg_count == 0 {
            return;
        }

        // ── Sliding highlight for the selected segment ──
        // The accent marks the selection and is contrast-checked against the track so the
        // selected label stays legible on either appearance. A caller's explicit text colour
        // still wins.
        let selected_bg = primary.contrast_color().blend(&primary, 0.30);
        let highlight_color = style.text_color.unwrap_or(selected_bg);
        let indicator_color = if self.base.is_enabled() {
            highlight_color
        } else {
            highlight_color.blend(&track_color, 0.50)
        };
        let sel_x = track_rect.x + (self.selected_index as i32) * seg_w;
        let sel_rect = Rect::new(
            sel_x + 2,
            track_rect.y + 2,
            seg_w.saturating_sub(4) as u32,
            track_rect.height.saturating_sub(4),
        );
        context.fill_rounded_rect(sel_rect, corner_radius, indicator_color);

        // ── Segment labels ──
        let font = Font::new("sans-serif", 13.0, false, false);
        // The unselected label is de-emphasised from the control's own ink rather than being
        // a fixed grey that a dark theme would render illegible.
        let unselected = ink.blend(&track_color, 0.45);
        for (i, seg) in self.segments.iter().enumerate() {
            let metrics = context.measure_text(seg, &font);
            let seg_x = track_rect.x + (i as i32) * seg_w;
            let text_x = seg_x + (seg_w - metrics.width as i32) / 2;
            // Centre the label in its segment. The origin is the glyph box's top edge, so the
            // offset is half the *line box*; the old ascent/descent pair began the glyph box
            // half a line below the segment's middle.
            let text_y = track_rect.y + (track_rect.height as i32 - metrics.height as i32) / 2;
            // The label must contrast with what it is painted on — the accent highlight for
            // the selected segment, the track for the rest — so the two are chosen against
            // their own backdrop instead of both assuming a light one.
            let color = if !self.base.is_enabled() {
                unselected.blend(&track_color, 0.50)
            } else if i == self.selected_index {
                indicator_color.contrast_color()
            } else {
                unselected
            };
            context.draw_text(
                Point::new(text_x, text_y),
                seg,
                &font,
                color,
                HorizontalAlignment::Left,
            );
        }
    }
}

impl EventHandler for CupertinoSegmentedControl {
    fn handle_event(&mut self, event: &Event) {
        match event {
            Event::MouseRelease { pos, button } => {
                // A disabled control must not change its selection. Without this the
                // `set_enabled(false)` API had no effect on this widget at all: the
                // click was still accepted and `value_changed` still fired.
                if !self.base.is_enabled() {
                    self.base.handle_event(event);
                    return;
                }
                if *button != 1 || self.segments.is_empty() {
                    return;
                }

                // The click is resolved against the same band the segments are painted in, so a
                // tap above or below the bar hits nothing rather than selecting a segment whose
                // ink is not there.
                let track_rect = self.track_band();
                let seg_w = track_rect.width as i32 / self.segments.len() as i32;
                if seg_w <= 0 {
                    return;
                }
                let rel_x = pos.x - track_rect.x;

                if rel_x < 0 || rel_x >= track_rect.width as i32 {
                    return;
                }
                if pos.y < track_rect.y || pos.y >= track_rect.y + track_rect.height as i32 {
                    return;
                }

                let index = (rel_x / seg_w) as usize;
                if index < self.segments.len() && index != self.selected_index {
                    self.selected_index = index;
                    self.value_changed.emit(index);
                    self.base.request_redraw();
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
    use crate::widget::svg::render_to_svg;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    #[test]
    fn cupertino_segmented_control_creation() {
        let sc = CupertinoSegmentedControl::new(Rect::new(0, 0, 300, 32));
        assert_eq!(sc.kind(), WidgetKind::CupertinoSegmentedControl);
        assert_eq!(sc.segment_count(), 0);
        assert_eq!(sc.selected_index(), 0);
    }

    #[test]
    fn cupertino_segmented_control_set_segments() {
        let mut sc = CupertinoSegmentedControl::new(Rect::new(0, 0, 300, 32));
        sc.set_segments(vec!["One".to_string(), "Two".to_string(), "Three".to_string()]);
        assert_eq!(sc.segment_count(), 3);
        assert_eq!(sc.selected_index(), 0);
    }

    #[test]
    fn cupertino_segmented_control_selection() {
        let mut sc = CupertinoSegmentedControl::new(Rect::new(0, 0, 300, 32));
        sc.set_segments(vec!["A".to_string(), "B".to_string(), "C".to_string()]);
        sc.set_selected_index(1);
        assert_eq!(sc.selected_index(), 1);

        // Clamp to max
        sc.set_selected_index(10);
        assert_eq!(sc.selected_index(), 2);
    }

    #[test]
    fn cupertino_segmented_control_value_changed_signal() {
        let mut sc = CupertinoSegmentedControl::new(Rect::new(0, 0, 300, 32));
        sc.set_segments(vec!["X".to_string(), "Y".to_string(), "Z".to_string()]);

        let fired = Arc::new(AtomicBool::new(false));
        let last_index = Arc::new(std::sync::Mutex::new(0usize));
        let f = fired.clone();
        let li = last_index.clone();
        sc.value_changed.connect(move |idx| {
            f.store(true, Ordering::SeqCst);
            *li.lock().unwrap() = *idx;
        });

        // Click on segment 1 (x ~100-199)
        // seg_w = 300/3 = 100, segment 1 is at x 100-199
        sc.handle_event(&Event::MouseRelease { pos: Point::new(150, 16), button: 1 });
        assert!(fired.load(Ordering::SeqCst));
        assert_eq!(*last_index.lock().unwrap(), 1);
    }

    #[test]
    fn cupertino_segmented_control_click_same_segment_no_emit() {
        let mut sc = CupertinoSegmentedControl::new(Rect::new(0, 0, 300, 32));
        sc.set_segments(vec!["A".to_string(), "B".to_string()]);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        sc.value_changed.connect(move |_: std::sync::Arc<usize>| {
            f.store(true, Ordering::SeqCst);
        });

        // Click on segment 0 (which is already selected)
        sc.handle_event(&Event::MouseRelease { pos: Point::new(50, 16), button: 1 });
        assert!(!fired.load(Ordering::SeqCst));
    }

    #[test]
    fn cupertino_segmented_control_svg_output() {
        let mut sc = CupertinoSegmentedControl::new(Rect::new(0, 0, 300, 32));
        sc.set_segments(vec!["Day".to_string(), "Week".to_string(), "Month".to_string()]);
        let svg = render_to_svg(&mut sc);
        assert!(svg.starts_with("<svg"));
    }

    /// A disabled segmented control must ignore taps.
    ///
    /// `handle_event` previously went straight to the hit test, so `set_enabled(false)`
    /// had no effect on this widget: a tap still moved the selection and still emitted
    /// `value_changed`. The neighbouring `LineEdit`/`Slider` gate on
    /// `is_enabled()` first, so this was an inconsistency rather than a deliberate
    /// policy.
    #[test]
    fn cupertino_segmented_control_disabled_ignores_taps() {
        let mut sc = CupertinoSegmentedControl::new(Rect::new(0, 0, 300, 32));
        sc.set_segments(vec!["A".to_string(), "B".to_string()]);
        sc.set_enabled(false);

        let fired = Arc::new(AtomicBool::new(false));
        let f = fired.clone();
        sc.value_changed.connect(move |_: std::sync::Arc<usize>| {
            f.store(true, Ordering::SeqCst);
        });

        // Segment 1 (x 150-299) is not the selected one, so an enabled control would
        // have changed selection here.
        sc.handle_event(&Event::MouseRelease { pos: Point::new(200, 16), button: 1 });

        assert!(!fired.load(Ordering::SeqCst), "a disabled control must not emit");
        assert_eq!(sc.selected_index(), 0, "a disabled control must not change selection");
    }

    /// A disabled control must paint differently from an enabled one.
    ///
    /// The paint path ignored `enabled` entirely, so a control that refused taps
    /// still looked fully interactive — the user taps and nothing happens.
    #[test]
    fn cupertino_segmented_control_disabled_renders_differently() {
        let mut sc = CupertinoSegmentedControl::new(Rect::new(0, 0, 300, 32));
        sc.set_segments(vec!["A".to_string(), "B".to_string()]);
        let enabled_svg = render_to_svg(&mut sc);

        sc.set_enabled(false);
        let disabled_svg = render_to_svg(&mut sc);

        assert_ne!(
            enabled_svg, disabled_svg,
            "the disabled state must be visible; otherwise the control lies about \
             accepting input"
        );
    }

    /// The track is one 32 px pill whatever rectangle the control was given.
    ///
    /// The defect this pins: the track was `fill_rounded_rect(rect, rect.height / 2, ..)`,
    /// so a 240x120 census cell drew a bare 240x120 stadium with no segment division in it —
    /// the segments inherited the same height, leaving nothing to divide.
    #[test]
    fn the_track_keeps_its_own_height_in_any_rectangle() {
        for height in [32u32, 60, 120, 300] {
            let sc = CupertinoSegmentedControl::new(Rect::new(0, 0, 240, height));
            assert_eq!(
                sc.track_band().height,
                crate::widget::metrics::dimensions::SEGMENTED_CONTROL_HEIGHT,
                "at control height {height}"
            );
        }
    }
}
