// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SegmentedButton widget — a Material 3-style segmented button group.
//!
//! The SegmentedButton widget displays a horizontal row of segments where the user
//! can select one (single-select) or multiple (multi-select) options. Each segment
//! displays a text label and an optional icon. The active/highlighted segment uses
//! a filled background to indicate selection.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
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

/// A single segment in a segmented button group.
#[derive(Debug, Clone)]
pub struct Segment {
    /// Stable identifier for this segment.
    pub id: String,
    /// Display label shown inside the segment.
    pub text: String,
    /// Optional icon name (displayed as a simple geometric shape).
    pub icon: Option<String>,
    /// Whether this segment is interactive.
    pub enabled: bool,
}

impl Segment {
    /// Creates a new segment with the given label text.
    pub fn new(text: &str) -> Self {
        Self { id: text.to_string(), text: text.to_string(), icon: None, enabled: true }
    }

    /// Sets the stable identifier for this segment.
    pub fn with_id(mut self, id: &str) -> Self {
        self.id = id.to_string();
        self
    }

    /// Sets an optional icon name for this segment.
    pub fn with_icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }

    /// Sets whether this segment is enabled.
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// SegmentedButton widget — a horizontal group of selectable segments.
///
/// Supports single-select (default) and multi-select modes. When a segment is
/// clicked, its selection toggles. In single-select mode, clicking a segment
/// selects it and deselects all others. The `selected_changed` signal emits
/// with the index of the newly selected segment.
pub struct SegmentedButton {
    base: BaseWidget,
    segments: Vec<Segment>,
    selected_index: Option<usize>,
    allows_multiple: bool,
    /// Emitted when the selected segment changes. Payload is the new selected index.
    pub selected_changed: Signal1<usize>,
}

impl SegmentedButton {
    /// Creates a new SegmentedButton widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SegmentedButton, geometry, "SegmentedButton"),
            segments: Vec::new(),
            selected_index: None,
            allows_multiple: false,
            selected_changed: Signal1::new(),
        }
    }

    /// Returns the index of the currently selected segment, or `None` if none is selected.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    /// Sets the selected segment index. Clamps to valid range.
    /// Pass `None` to deselect all. In single-select mode, only one segment can be selected.
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        let old = self.selected_index;
        self.selected_index = index.filter(|i| *i < self.segments.len());
        if old != self.selected_index {
            if let Some(idx) = self.selected_index {
                self.selected_changed.emit(idx);
            }
            self.base.request_redraw();
        }
    }

    /// Returns whether multiple segments can be selected at once.
    pub fn allows_multiple(&self) -> bool {
        self.allows_multiple
    }

    /// Sets whether multiple segments can be selected.
    pub fn set_allows_multiple(&mut self, allows: bool) {
        self.allows_multiple = allows;
        if !allows && self.segments.len() > 1 {
            // In single-select mode, deselect all but the first selected
            self.selected_index = self.selected_index.filter(|i| *i < self.segments.len());
        }
        self.base.request_redraw();
    }

    /// Returns the list of segments.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    /// Returns a mutable reference to the list of segments.
    pub fn segments_mut(&mut self) -> &mut Vec<Segment> {
        &mut self.segments
    }

    /// Adds a segment to the end of the group.
    pub fn add_segment(&mut self, segment: Segment) {
        self.segments.push(segment);
        self.base.request_redraw();
    }

    /// Removes a segment by index. Returns the removed segment if the index was valid.
    pub fn remove_segment(&mut self, index: usize) -> Option<Segment> {
        if index < self.segments.len() {
            let removed = self.segments.remove(index);
            // Adjust selected index
            if let Some(sel) = self.selected_index {
                if sel == index {
                    self.selected_index = None;
                } else if sel > index {
                    self.selected_index = Some(sel - 1);
                }
            }
            self.base.request_redraw();
            Some(removed)
        } else {
            None
        }
    }

    /// Returns the number of segments.
    pub fn segment_count(&self) -> usize {
        self.segments.len()
    }

    /// Checks whether the given point falls within a segment's hit area.
    ///
    /// Resolved against the same track band the segments are painted in, so a click above or
    /// below the bar hits nothing rather than selecting a segment whose ink is not there.
    fn hit_segment(&self, pos: Point) -> Option<usize> {
        if self.segments.is_empty() {
            return None;
        }
        let band = self.track_band();
        if pos.y < band.y || pos.y >= band.y + band.height as i32 {
            return None;
        }
        if pos.x < band.x || pos.x >= band.x + band.width as i32 {
            return None;
        }
        let seg_width = band.width / self.segments.len() as u32;
        if seg_width == 0 {
            return None;
        }
        let local_x = (pos.x - band.x) as u32;
        let index = (local_x / seg_width) as usize;
        if index < self.segments.len() {
            Some(index)
        } else {
            None
        }
    }

    /// The track the control actually paints: full width,
    /// `dimensions::SEGMENTED_CONTROL_HEIGHT` tall, centred in the rectangle it was given.
    ///
    /// # Why the track is not the rectangle
    ///
    /// A segmented button's chrome is one row of segments, not a filled container. Taking
    /// `rect.height` made a 240x120 census cell a **240x120 rounded rectangle** whose corner
    /// radius became 16 — a slab shaped like a segmented button rather than one — and it
    /// disagreed with the 32 px `size_hint` the control reports. Deriving the band once here
    /// keeps the paint, the hit test and the reported size on one value.
    fn track_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::SEGMENTED_CONTROL_HEIGHT)
    }
}

impl Widget for SegmentedButton {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 32)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `SegmentedButton`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `selected_index` reads back as
/// the real selection (or `Null` when nothing is selected) rather than the legacy
/// hardcoded zero, and `segment_count` is derived from the segment list, so it is
/// readable but read-only.
impl WidgetProperties for SegmentedButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "segment_count" => Ok(CapabilityValue::UInt(self.segment_count() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "selected_index" => match value {
                // A `Null` write means "clear the selection", mirroring the
                // `Option<usize>` the widget's own setter takes.
                CapabilityValue::Null => {
                    self.set_selected_index(None);
                    Ok(())
                }
                other => {
                    self.set_selected_index(Some(expect_usize(other)?));
                    Ok(())
                }
            },
            "segment_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["selected_index", "segment_count", BASE_PROPERTY_NAMES]
    }
}

impl Draw for SegmentedButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so a
        // light/dark switch left the control unchanged — the rendering census reported it as
        // theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("segmented_button");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, secondary, primary) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.secondary,
                    active.colors.primary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(158, 158, 158),
                    Color::rgb(33, 150, 243),
                ),
            }
        };
        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // `segmented_button` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and the active theme writes the window fill into `style.background_color`.
        // An empty track painted in that colour would be byte-identical to the frame behind it, so
        // a resolved surface equal to the window fill is re-derived a visible step away from it,
        // while a colour the caller set still wins.
        let track = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => window_fill.blend(&ink, 0.10),
        };
        let border_color = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .filter(|resolved| *resolved != track)
            .unwrap_or_else(|| track.blend(&secondary, 0.55));

        let is_enabled = self.base.is_enabled();
        let seg_count = self.segments.len();
        // ── The track actually painted ──
        //
        // The corner radius is derived from the track's own height (half of it, floored at 4),
        // so a 32 px track has 16 px ends and a track clamped smaller keeps a proportionate
        // rounding rather than the 16 px radius a 120 px canvas produced.
        let track_rect = self.track_band();
        let corner_radius = (track_rect.height / 2).max(4);
        let font = Font::simple("sans-serif", 13.0);

        // The track is painted before the early return on an empty segment list, so a freshly
        // constructed control is visible rather than reporting `ink = 0`. An empty track reads as
        // a disabled group, which is exactly what a segmented button with nothing to choose is.
        context.fill_rounded_rect(track_rect, corner_radius, track);
        // Draw outer container border
        let border_color = if is_enabled { border_color } else { border_color.with_alpha(100) };
        context.draw_rounded_rect_stroke(track_rect, corner_radius, border_color, 1);

        if seg_count == 0 {
            return;
        }

        let seg_width = track_rect.width / seg_count as u32;

        for (i, segment) in self.segments.iter().enumerate() {
            let seg_rect = Rect::new(
                track_rect.x + (i as u32 * seg_width) as i32,
                track_rect.y,
                seg_width,
                track_rect.height,
            );

            let is_selected = self.selected_index == Some(i);
            let seg_enabled = is_enabled && segment.enabled;

            // Determine colors. A selected segment carries the theme's primary so the selection is
            // visible on either appearance rather than a fixed Material blue.
            let bg_color = if !seg_enabled {
                track
            } else if is_selected {
                primary
            } else {
                track
            };

            // Draw segment background
            if is_selected {
                if seg_count == 1 {
                    context.fill_rounded_rect(seg_rect, corner_radius, bg_color);
                } else if i == 0 {
                    // First segment: round left corners only
                    context.fill_rounded_rect(seg_rect, corner_radius, bg_color);
                    // Over-draw right side square
                    let right_half = Rect::new(
                        seg_rect.x + corner_radius as i32,
                        seg_rect.y,
                        seg_width - corner_radius,
                        track_rect.height,
                    );
                    context.fill_rect(right_half, bg_color);
                } else if i == seg_count - 1 {
                    // Last segment: round right corners only
                    context.fill_rounded_rect(seg_rect, corner_radius, bg_color);
                    // Over-draw left side square
                    let left_half = Rect::new(
                        seg_rect.x,
                        seg_rect.y,
                        seg_width - corner_radius,
                        track_rect.height,
                    );
                    context.fill_rect(left_half, bg_color);
                } else {
                    // Middle segments: fill fully
                    context.fill_rect(seg_rect, bg_color);
                }
            }

            // Draw vertical divider between segments
            if i > 0 {
                let divider_x = seg_rect.x;
                let divider_color =
                    if is_enabled { border_color } else { border_color.with_alpha(80) };
                context.draw_line(
                    Point::new(divider_x, seg_rect.y + 4),
                    Point::new(divider_x, seg_rect.y + track_rect.height as i32 - 4),
                    divider_color,
                );
            }

            // Draw text centered in segment. A disabled label is dimmed, and a selected one takes
            // the contrast colour of the primary it sits on.
            let text_color = if !seg_enabled {
                ink.blend(&track, 0.60)
            } else if is_selected {
                primary.contrast_color()
            } else {
                ink
            };

            let metrics = context.measure_text(&segment.text, &font);
            let text_x = seg_rect.x + (seg_width as i32 - metrics.width as i32) / 2;
            // The origin is the glyph box's top edge, so centring on the segment is half the
            // *line box*; the old `+ ascent` began the box half a line below the middle.
            let text_y = seg_rect.y + (track_rect.height as i32 - metrics.height as i32) / 2;

            if !segment.text.is_empty() {
                context.draw_text(
                    Point::new(text_x.max(seg_rect.x), text_y.max(seg_rect.y)),
                    &segment.text,
                    &font,
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

impl EventHandler for SegmentedButton {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            return;
        }
        match event {
            Event::MousePress { pos, button } => {
                if *button == 1 {
                    if let Some(index) = self.hit_segment(*pos) {
                        if index < self.segments.len() && self.segments[index].enabled {
                            if self.allows_multiple {
                                // In multi-select, toggle the selection
                                if self.selected_index == Some(index) {
                                    self.selected_index = None;
                                } else {
                                    self.selected_index = Some(index);
                                }
                                self.selected_changed.emit(index);
                            } else {
                                // Single-select: always set to clicked segment
                                self.selected_index = Some(index);
                                self.selected_changed.emit(index);
                            }
                            self.base.request_redraw();
                        }
                    }
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

    #[test]
    fn segmented_button_default_creation() {
        let btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        assert_eq!(btn.kind(), WidgetKind::SegmentedButton);
        assert_eq!(btn.selected_index(), None);
        assert!(!btn.allows_multiple());
        assert_eq!(btn.segment_count(), 0);
    }

    #[test]
    fn segmented_button_add_and_select_segment() {
        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        btn.add_segment(Segment::new("Day"));
        btn.add_segment(Segment::new("Week"));
        btn.add_segment(Segment::new("Month"));
        assert_eq!(btn.segment_count(), 3);

        btn.set_selected_index(Some(1));
        assert_eq!(btn.selected_index(), Some(1));

        // Remove middle segment
        let removed = btn.remove_segment(1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().text, "Week");
        assert_eq!(btn.segment_count(), 2);
    }

    #[test]
    fn segmented_button_selected_changed_signal() {
        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        btn.add_segment(Segment::new("A"));
        btn.add_segment(Segment::new("B"));

        let captured = Arc::new(Mutex::new(None));
        btn.selected_changed.connect({
            let captured = Arc::clone(&captured);
            move |val: Arc<usize>| {
                *captured.lock().unwrap() = Some(*val);
            }
        });

        // Click on second segment
        btn.handle_event(&Event::mouse_press(200, 18, 1));
        assert_eq!(btn.selected_index(), Some(1));
        assert_eq!(*captured.lock().unwrap(), Some(1));
    }

    #[test]
    fn segmented_button_disabled_segment_ignores_clicks() {
        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        btn.add_segment(Segment::new("One"));
        let mut seg = Segment::new("Two");
        seg.enabled = false;
        btn.add_segment(seg);

        // Click on disabled segment (index 1, x position ~150-300)
        btn.handle_event(&Event::mouse_press(200, 18, 1));
        assert_eq!(btn.selected_index(), None);
    }

    #[test]
    fn segmented_button_svg_output() {
        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        btn.add_segment(Segment::new("Left"));
        btn.add_segment(Segment::new("Center"));
        btn.add_segment(Segment::new("Right"));
        btn.set_selected_index(Some(1));

        let svg = crate::widget::svg::render_to_svg(&mut btn);
        assert!(svg.starts_with("<svg"));
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn segmented_button_remove_adjusts_selection() {
        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        btn.add_segment(Segment::new("X"));
        btn.add_segment(Segment::new("Y"));
        btn.add_segment(Segment::new("Z"));
        btn.set_selected_index(Some(2));

        // Remove last segment (index 2)
        btn.remove_segment(2);
        assert_eq!(btn.selected_index(), None);

        btn.set_selected_index(Some(0));
        // Remove first segment — selection shifts
        btn.remove_segment(0);
        assert_eq!(btn.selected_index(), None);
    }

    #[test]
    fn segmented_button_click_outside_does_nothing() {
        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        btn.add_segment(Segment::new("Test"));

        btn.handle_event(&Event::mouse_press(500, 500, 1));
        assert_eq!(btn.selected_index(), None);
    }

    /// The track is one row tall whatever rectangle the control was given.
    ///
    /// The defect this pins: the track and its segments were sized from `rect.height`, so a
    /// 240x120 census cell drew a 240x120 rounded rectangle (radius 16) whose segments were
    /// also 120 tall — a slab shaped like a segmented button rather than one.
    #[test]
    fn the_track_keeps_its_own_height_in_any_rectangle() {
        for height in [32u32, 60, 120, 300] {
            let btn = SegmentedButton::new(Rect::new(0, 0, 240, height));
            assert_eq!(
                btn.track_band().height,
                dimensions::SEGMENTED_CONTROL_HEIGHT,
                "at control height {height}"
            );
        }
    }
}
