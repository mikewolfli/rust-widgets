// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Breadcrumb navigation widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Single breadcrumb segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BreadcrumbSegment {
    /// Stable segment identifier.
    pub id: String,
    /// Visible segment label.
    pub label: String,
}

impl BreadcrumbSegment {
    /// Creates a breadcrumb segment.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into() }
    }
}

/// Breadcrumb navigation control with keyboard and mouse interaction.
pub struct Breadcrumb {
    base: BaseWidget,
    segments: Vec<BreadcrumbSegment>,
    selected_index: Option<usize>,
    segment_padding: i32,
    separator_width: i32,
    /// Emitted when a segment is activated. Payload is segment id.
    pub segment_activated: Signal1<String>,
}

impl Breadcrumb {
    /// Creates an empty breadcrumb.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Breadcrumb, geometry, "Breadcrumb"),
            segments: Vec::new(),
            selected_index: None,
            segment_padding: 8,
            separator_width: 14,
            segment_activated: Signal1::new(),
        }
    }

    /// Replaces full segment path.
    pub fn set_segments(&mut self, segments: Vec<BreadcrumbSegment>) {
        self.segments = segments;
        self.selected_index =
            if self.segments.is_empty() { None } else { Some(self.segments.len() - 1) };
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns immutable segment list.
    pub fn segments(&self) -> &[BreadcrumbSegment] {
        &self.segments
    }

    /// Appends one segment.
    pub fn push_segment(&mut self, segment: BreadcrumbSegment) {
        self.segments.push(segment);
        self.selected_index = Some(self.segments.len() - 1);
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears segment path.
    pub fn clear_segments(&mut self) {
        self.segments.clear();
        self.selected_index = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns selected segment index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index.filter(|index| *index < self.segments.len())
    }

    /// Sets selected segment.
    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if index >= self.segments.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        self.selected_index = Some(index);
        self.base.request_redraw();
        true
    }

    /// Activates currently selected segment.
    pub fn activate_selected(&mut self) -> bool {
        let Some(index) = self.selected_index() else {
            return false;
        };
        let Some(segment) = self.segments.get(index) else {
            return false;
        };
        self.segment_activated.emit(segment.id.clone());
        true
    }

    /// Moves selection by signed delta.
    pub fn move_selection(&mut self, delta: isize) {
        if self.segments.is_empty() {
            self.selected_index = None;
            return;
        }
        let current = self.selected_index.unwrap_or(0) as isize;
        let max = self.segments.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        self.selected_index = Some(next);
        self.base.request_redraw();
    }

    fn segment_width(segment: &BreadcrumbSegment, padding: i32) -> i32 {
        (segment.label.chars().count() as i32) * 8 + padding * 2
    }

    /// The row the trail occupies: full width,
    /// `dimensions::BREADCRUMB_HEIGHT` tall, centred in the control's rectangle.
    ///
    /// # Why the trail has its own height
    ///
    /// A breadcrumb is chrome: one compact row of links. Taking `rect.height` made a 240x120
    /// census cell a 120 px-tall trail whose selected segment was a full-height column, and it
    /// disagreed with the 28 px `size_hint` the control reports. The band is the single
    /// derivation the paint and the hit test share.
    fn band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::BREADCRUMB_HEIGHT)
    }

    /// The rectangle of segment `index`, laid out from the leading edge of [`Self::band`].
    ///
    /// One derivation for both the paint and the hit test: they used to place the separator
    /// differently — the draw loop advanced past it only between segments, the hit test
    /// unconditionally — so the last segment's ink and its hit box disagreed by one separator
    /// width. A segment past the band's right edge is clamped to it, because nothing clips a
    /// widget at this layer.
    fn segment_rect_at(&self, index: usize) -> Option<Rect> {
        if index >= self.segments.len() {
            return None;
        }
        let band = self.band();
        let band_right = band.x + band.width as i32;
        let mut x = band.x;

        for (i, segment) in self.segments.iter().enumerate() {
            if x >= band_right {
                break;
            }
            let width = Self::segment_width(segment, self.segment_padding).max(1);
            if i == index {
                let visible_width = width.min(band_right - x).max(0) as u32;
                if visible_width == 0 {
                    return None;
                }
                return Some(Rect::new(x, band.y, visible_width, band.height));
            }
            x += width;
            // The separator belongs to the gap *between* segments, so it is reserved only when
            // another segment follows — the rule the paint and the hit test now both use.
            if i + 1 < self.segments.len() {
                x += self.separator_width;
            }
        }

        None
    }

    fn hit_index(&self, pos: Point) -> Option<usize> {
        let band = self.band();
        if pos.y < band.y || pos.y >= band.y + band.height as i32 {
            return None;
        }
        if pos.x < band.x || pos.x >= band.x + band.width as i32 {
            return None;
        }

        for index in 0..self.segments.len() {
            let Some(seg_rect) = self.segment_rect_at(index) else {
                continue;
            };
            if pos.x >= seg_rect.x && pos.x < seg_rect.x + seg_rect.width as i32 {
                return Some(index);
            }
        }

        None
    }
}

impl Widget for Breadcrumb {
    fn base(&self) -> &BaseWidget {
        &self.base
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(300, 28)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
}

/// `Breadcrumb`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_base.in.rs` dispatch. Both properties are derived counts of the
/// segment path, so writes are refused with
/// [`CapabilityAccessError::ReadOnlyProperty`]: the path is changed through
/// `set_segments` / `push_segment` / `clear_segments`, not one index at a time.
impl WidgetProperties for Breadcrumb {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "segment_count" => Ok(CapabilityValue::UInt(self.segments().len() as u64)),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "segment_count" | "selected_index" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["segment_count", "selected_index", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `breadcrumb` publishes.
    ///
    /// `clear_segments` discards the path and needs no argument, so a bare
    /// invocation performs it. `push_segment` needs the [`BreadcrumbSegment`] to
    /// append, so it is [`CapabilityAccessError::OutOfRange`] — the name is right and
    /// the payload is missing — while `set_segments` carries the whole path and is
    /// answered through the same refusal.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "clear_segments" => {
                self.clear_segments();
                Ok(())
            }
            "push_segment" | "set_segments" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Breadcrumb {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MousePress { pos, button: 1 } => {
                if let Some(index) = self.hit_index(*pos) {
                    let _ = self.set_selected_index(index);
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } => {
                if let Some(index) = self.hit_index(*pos) {
                    let _ = self.set_selected_index(index);
                    let _ = self.activate_selected();
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => self.move_selection(-1),
                39 => self.move_selection(1),
                13 => {
                    let _ = self.activate_selected();
                }
                _ => { /* Other keys are not relevant */ }
            },
            _ => { /* Other events are not relevant */ }
        }
    }
}

impl Draw for Breadcrumb {
    fn draw(&mut self, context: &mut RenderContext) {
        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then fall back to a literal. Without the
        // theme step a light/dark switch would change nothing on screen because
        // every colour below was previously hardcoded.
        //
        // The theme reads are separate manager locks, each taken and released inside
        // `resolved_theme_style` (or the scoped block below), so none is held across
        // the draw or across another accessor — the global manager's mutex is not
        // re-entrant.
        let style = self.base.style().clone();
        let theme = crate::style::resolved_theme_style("breadcrumb");
        // `breadcrumb` resolves to the `Text` role, which carries a foreground but no
        // surface — a text control sits on whatever it was placed on. So the surface
        // comes from the theme's own background, which is what
        // `apply_active_theme` could not supply and what a light/dark switch changes
        // most. The guard is scoped to this expression and released before any other
        // theme accessor runs.
        let theme_surface =
            crate::style::theme_manager().current_theme().map(|theme| theme.colors.background);
        let background = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .or(theme_surface)
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| background.blend(&Color::BLACK, 0.15));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::BLACK);
        // The selected segment and the separators are secondary chrome: derived from
        // the resolved colours so they move with the appearance too.
        let selected_background = background.blend(&text_color, 0.12);
        let separator_color = text_color.blend(&background, 0.45);

        // ── The trail actually painted ──
        //
        // `rect` is the area the control was *given*; a breadcrumb trail is one compact row of
        // links, `dimensions::BREADCRUMB_HEIGHT` tall and centred in that area. Filling the
        // whole rectangle made a 240x120 census cell a 120 px-tall trail whose selected segment
        // was a full-height column — a slab shaped like a breadcrumb rather than a breadcrumb.
        // The band is what the segments below subdivide, and the same derivation the hit test
        // reads, so ink and hit box cannot disagree.
        let band = self.band();
        context.fill_rect(band, background);
        context.draw_rect(band, border);

        for (index, segment) in self.segments.iter().enumerate() {
            let Some(segment_rect) = self.segment_rect_at(index) else {
                continue;
            };

            if self.selected_index == Some(index) {
                context.fill_rect(segment_rect, selected_background);
            }

            // A label's origin is the glyph box's top edge, so the segment's raw midpoint put
            // that edge on the middle line and drew the label half a line low. The line box
            // centred in the segment gives the origin instead; the separator below shares it.
            let line = context.text_line(segment_rect, &Font::default());
            context.draw_text(
                Point::new(segment_rect.x + self.segment_padding, line.y),
                &segment.label,
                &Font::default(),
                text_color,
                HorizontalAlignment::Left,
            );

            // The separator sits in the gap after the segment, and only exists when another
            // segment follows it inside the band.
            let gap_x = segment_rect.x + segment_rect.width as i32;
            let next = self.segments.get(index + 1);
            if next.is_some() && gap_x + self.separator_width <= band.x + band.width as i32 {
                context.draw_text(
                    Point::new(gap_x + 3, line.y),
                    ">",
                    &Font::default(),
                    separator_color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_segments() -> Vec<BreadcrumbSegment> {
        vec![
            BreadcrumbSegment::new("root", "Root"),
            BreadcrumbSegment::new("project", "Project"),
            BreadcrumbSegment::new("src", "src"),
        ]
    }

    #[test]
    fn set_segments_selects_last_by_default() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 320, 28));
        breadcrumb.set_segments(sample_segments());

        assert_eq!(breadcrumb.selected_index(), Some(2));
        assert_eq!(breadcrumb.segments().len(), 3);
    }

    #[test]
    fn keyboard_navigation_and_activation_emit_segment_id() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 320, 28));
        breadcrumb.set_segments(sample_segments());

        let activated = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = activated.clone();
        breadcrumb.segment_activated.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        breadcrumb.handle_event(&Event::key_press(37, 0));
        assert_eq!(breadcrumb.selected_index(), Some(1));
        breadcrumb.handle_event(&Event::key_press(13, 0));

        let got = activated.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["project".to_string()]);
    }

    #[test]
    fn mouse_selection_hits_expected_segment() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 400, 28));
        breadcrumb.set_segments(sample_segments());

        // Root label area
        breadcrumb.handle_event(&Event::mouse_press(12, 10, 1));
        assert_eq!(breadcrumb.selected_index(), Some(0));

        // Project label area after first separator
        breadcrumb.handle_event(&Event::mouse_press(90, 10, 1));
        assert_eq!(breadcrumb.selected_index(), Some(1));
    }

    #[test]
    fn default_state() {
        let breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 600));
        assert!(breadcrumb.segments().is_empty());
        assert_eq!(breadcrumb.selected_index(), None);
    }

    #[test]
    fn push_segment_increases_count() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 600));
        breadcrumb.push_segment(BreadcrumbSegment::new("a", "A"));
        assert_eq!(breadcrumb.segments().len(), 1);
        assert_eq!(breadcrumb.selected_index(), Some(0));

        breadcrumb.push_segment(BreadcrumbSegment::new("b", "B"));
        assert_eq!(breadcrumb.segments().len(), 2);
        assert_eq!(breadcrumb.selected_index(), Some(1));

        breadcrumb.push_segment(BreadcrumbSegment::new("c", "C"));
        assert_eq!(breadcrumb.segments().len(), 3);
        assert_eq!(breadcrumb.selected_index(), Some(2));
    }

    #[test]
    fn clear_segments_resets() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 600));
        breadcrumb
            .set_segments(vec![BreadcrumbSegment::new("a", "A"), BreadcrumbSegment::new("b", "B")]);
        assert_eq!(breadcrumb.segments().len(), 2);

        breadcrumb.clear_segments();
        assert!(breadcrumb.segments().is_empty());
        assert_eq!(breadcrumb.selected_index(), None);
    }

    #[test]
    fn empty_breadcrumb_state() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 600));
        // activate on empty = false
        assert!(!breadcrumb.activate_selected());

        // move selection on empty should not panic
        breadcrumb.move_selection(1);
        assert_eq!(breadcrumb.selected_index(), None);

        breadcrumb.move_selection(-1);
        assert_eq!(breadcrumb.selected_index(), None);

        // set_selected_index on empty = false
        assert!(!breadcrumb.set_selected_index(0));
    }

    #[test]
    fn invalid_segment_activation_index() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 600));
        breadcrumb.set_segments(vec![BreadcrumbSegment::new("a", "A")]);

        // Setting index beyond bounds returns false
        assert!(!breadcrumb.set_selected_index(5));

        // Setting valid index returns true
        assert!(breadcrumb.set_selected_index(0));

        // Re-setting same index returns true
        assert!(breadcrumb.set_selected_index(0));
    }

    /// A trail is one compact row whatever height the control was given.
    ///
    /// The defect this pins: the trail and its segments were sized from `rect`, so a 240x120
    /// census cell drew a 120 px-tall trail whose selected segment was a full-height column.
    /// The row's height is chrome.
    #[test]
    fn the_trail_keeps_its_own_height_in_any_rectangle() {
        for height in [28u32, 60, 120, 300] {
            let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 320, height));
            breadcrumb.set_segments(sample_segments());
            let band = breadcrumb.band();
            assert_eq!(band.height, crate::widget::metrics::dimensions::BREADCRUMB_HEIGHT);
            let seg = breadcrumb.segment_rect_at(0).expect("a laid-out segment");
            assert_eq!(seg.height, band.height, "a segment fills the trail's row, not the control");
        }
    }
}
