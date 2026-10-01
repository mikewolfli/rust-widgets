// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SegmentedControl widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, ControlMetrics};
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The horizontal inset a segment's label takes from its own edge: 16.
///
/// Material's segmented control (`ButtonSegment`'s `padding`) insets a segment's label by 16 px. It
/// was `8`, which put a short label close enough to its divider that the two read as one mark.
const SEGMENT_LABEL_INSET: i32 = 16;

/// Single segment entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentItem {
    /// Stable item id.
    pub id: String,
    /// Display label.
    pub label: String,
}

impl SegmentItem {
    /// Creates one segment item.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into() }
    }
}

/// Single-selection segmented control.
pub struct SegmentedControl {
    base: BaseWidget,
    items: Vec<SegmentItem>,
    selected_index: Option<usize>,
    hovered_index: Option<usize>,
    /// The **drawn** position of the selection indicator, as a 0.0..=1.0 fraction of the way from
    /// the segment it left to the segment it is heading to.
    ///
    /// # Why the indicator is not drawn at `selected_index`
    ///
    /// The selection is the logical answer a caller reads the instant it changes; the indicator's
    /// position is what the reader sees. Drawing the pill at `selected_index` moves it between two
    /// adjacent frames with nothing in between, so a three-way switch reads as a jump rather than
    /// as a control changing its value. Interpolating between the old and new segment makes the
    /// movement itself the information. Same split, same reason, as `Switch`'s `checked`/`travel`.
    ///
    /// # Why a fraction rather than an index
    ///
    /// A `PropertyDriver` interpolates a 0..=1 progress, not an arbitrary value (it clamps its
    /// target to that range, because a progress past 100% is not a number any caller wants).
    /// Expressing the indicator as "how far between two segments" keeps it inside that contract and
    /// makes the endpoints — which segment the pill is leaving, which it is arriving at — the two
    /// arguments of the interpolation rather than two stored copies of the state.
    slide: PropertyDriver,
    /// The segment the current slide started from, so `slide`'s fraction has a left endpoint.
    slide_from: usize,
    /// Emitted when selected segment changes. Payload is selected id.
    pub selection_changed: Signal1<String>,
}

impl SegmentedControl {
    /// Creates an empty segmented control.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SegmentedControl, geometry, "SegmentedControl"),
            items: Vec::new(),
            selected_index: None,
            hovered_index: None,
            // A fresh control rests *on* the first segment, which is `slide == 1.0`: the fraction
            // measures how far from `slide_from` (0) the pill has arrived, not how far it has left.
            slide: PropertyDriver::at(1.0, MotionSlot::Normal),
            slide_from: 0,
            selection_changed: Signal1::new(),
        }
    }

    /// Replaces all segment items.
    pub fn set_items(&mut self, items: Vec<SegmentItem>) {
        self.items = items;
        self.selected_index = if self.items.is_empty() { None } else { Some(0) };
        self.hovered_index = self.selected_index;
        // A new item set is a *reset*, not a selection change: the indicator starts on the first
        // segment rather than sliding there from wherever the old set left it.
        self.slide.jump_to(1.0);
        self.slide_from = 0;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns all segment items.
    pub fn items(&self) -> &[SegmentItem] {
        &self.items
    }

    /// Returns selected segment index.
    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index.filter(|index| *index < self.items.len())
    }

    /// Returns selected segment id.
    pub fn selected_id(&self) -> Option<&str> {
        let index = self.selected_index()?;
        self.items.get(index).map(|item| item.id.as_str())
    }

    /// Sets selected segment index.
    pub fn set_selected_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        // The re-aim has to preserve the pill's **fractional** position, not round it to a segment:
        // a user who clicks segment 2 while the pill is 40% of the way to segment 1 would otherwise
        // see it snap back to segment 0 first, which is the one thing an indicator exists to avoid.
        // The old endpoints are resolved into a position, then that position becomes the new slide's
        // starting fraction against the newly chosen target.
        let position = self.indicator_position();
        self.slide_from = position.floor().max(0.0) as usize;
        let from = self.slide_from as f32;
        let to = index as f32;
        let fraction = if (to - from).abs() < f32::EPSILON {
            1.0
        } else {
            ((position - from) / (to - from)).clamp(0.0, 1.0)
        };
        self.selected_index = Some(index);
        self.slide.jump_to(fraction);
        self.slide.set_target(1.0);
        if let Some(item) = self.items.get(index) {
            self.selection_changed.emit(item.id.clone());
        }
        self.base.request_redraw();
        true
    }

    /// The segment index the indicator is currently drawn at, as a **fractional** value.
    ///
    /// This is the value an animation test samples: it is strictly between two whole indices while
    /// the pill is travelling, which is exactly what "the indicator slid rather than jumped" means.
    pub fn indicator_position(&self) -> f32 {
        let from = self.slide_from as f32;
        let to = self.selected_index.unwrap_or(self.slide_from) as f32;
        from + (to - from) * self.slide.value()
    }

    /// Advances the indicator's slide by `delta_ms`; `true` while it is still moving.
    pub fn tick(&mut self, delta_ms: u32) -> bool {
        self.slide.tick(delta_ms)
    }

    /// Whether the indicator is between two segments -- answers only, never advances.
    pub fn is_animating(&self) -> bool {
        self.slide.is_moving()
    }

    /// Moves selection by signed delta.
    pub fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.selected_index = None;
            return;
        }
        let current = self.selected_index.unwrap_or(0) as isize;
        let max = self.items.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        let _ = self.set_selected_index(next);
    }

    fn segment_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.items.len() || self.items.is_empty() {
            return None;
        }
        let band = self.track_band();
        let width = (band.width as usize / self.items.len()).max(1) as u32;
        let x = band.x + index as i32 * width as i32;
        let mut actual_width = width;
        if index + 1 == self.items.len() {
            let consumed = width.saturating_mul(index as u32);
            actual_width = band.width.saturating_sub(consumed);
        }
        Some(Rect::new(x, band.y, actual_width, band.height))
    }

    /// The bar the control actually paints: full width, `dimensions::SEGMENTED_CONTROL_HEIGHT`
    /// tall, centred in the rectangle it was given.
    ///
    /// # Why the bar is not the rectangle
    ///
    /// A segmented control's chrome is one row of segments, not a filled container. Taking
    /// `rect.height` made a 240x120 census cell a **240x120 bar** whose segments were also
    /// 120 tall — a rectangle shaped like a segmented control rather than one — and it made
    /// the drawn pill disagree with the 32 px `size_hint` the control reports. Deriving the
    /// band once here is what keeps the paint, the hit test and the reported size on one
    /// value.
    fn track_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::SEGMENTED_CONTROL_HEIGHT)
    }

    fn hit_index(&self, pos: Point) -> Option<usize> {
        let band = self.track_band();
        if pos.x < band.x
            || pos.x >= band.x + band.width as i32
            || pos.y < band.y
            || pos.y >= band.y + band.height as i32
        {
            return None;
        }

        for index in 0..self.items.len() {
            let Some(seg) = self.segment_rect(index) else {
                continue;
            };
            if pos.x >= seg.x && pos.x < seg.x + seg.width as i32 {
                return Some(index);
            }
        }
        None
    }

    /// The segment rectangle the indicator is sliding **from**.
    fn indicator_from(&self) -> Option<Rect> {
        self.segment_rect(self.slide_from)
    }

    /// The segment rectangle the indicator is sliding **to**.
    fn indicator_to(&self) -> Option<Rect> {
        self.segment_rect(self.selected_index.unwrap_or(self.slide_from))
    }
}

impl Widget for SegmentedControl {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(
            crate::widget::metrics::dimensions::SEGMENTED_CONTROL_DEFAULT_WIDTH,
            crate::widget::metrics::dimensions::SEGMENTED_CONTROL_HEIGHT,
        )
    }

    // The indicator slide is the control's own animation; the trait spelling is what the frame bus
    // reaches through `&mut dyn Widget`, which is the only way the slide actually happens.
    fn tick(&mut self, delta_ms: u32) -> bool {
        SegmentedControl::tick(self, delta_ms)
    }

    fn is_animating(&self) -> bool {
        SegmentedControl::is_animating(self)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit per control
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually emits. Without it a name
    /// is valid and inert, which is the silent failure `tools/check_event_signal_dyn.sh` exists to
    /// make impossible. The arm set is checked against the capability's published names, so this
    /// list cannot drift from what the control advertises.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        #[allow(unused_imports)]
        use crate::widget::capability::CapabilityValue;
        match name {
            "selection_changed" => {
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |_| {
                    CapabilityValue::Null
                }))
            }
            _ => None,
        }
    }
}

/// `SegmentedControl`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_other.in.rs` dispatch. All three properties describe the item set
/// and the current selection and have no writers in the legacy path, so `set`
/// refuses them with [`CapabilityAccessError::ReadOnlyProperty`] rather than
/// pretending the name does not exist. `SegmentedControl` reports
/// `WidgetKind::ToggleButton`, shared with `ToggleButton`; dispatching on the
/// concrete type here is what keeps the two contracts separate.
impl WidgetProperties for SegmentedControl {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "selected_id" => match self.selected_id() {
                Some(id) => Ok(CapabilityValue::String(id.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "item_count" | "selected_index" | "selected_id" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of!["item_count", "selected_index", "selected_id", BASE_PROPERTY_NAMES]
    }

    /// Runs one of the commands `segmented_control` publishes.
    ///
    /// `move_selection` takes a signed delta, and the affordance the control itself
    /// offers for a bare move is "advance one": its right-arrow handler calls
    /// `move_selection(1)`. There is no way to express a direction without an
    /// argument, so rather than guess one — which would silently do something the
    /// caller did not ask for — the name is refused as
    /// [`CapabilityAccessError::OutOfRange`], meaning the name is valid and the
    /// argument is what is missing. `set_items` carries the item list and is refused
    /// for the same reason.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "move_selection" | "set_items" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for SegmentedControl {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                // Hover is painted (the hovered segment takes `hovered_bg`), so it must repaint —
                // and only when the hovered segment actually changes, since `MouseMove` fires far
                // more often than the highlight moves. Without this the highlight lagged until an
                // unrelated event repainted.
                let hovered = self.hit_index(*pos);
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
                if let Some(index) = self.hit_index(*pos) {
                    let _ = self.set_selected_index(index);
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => self.move_selection(-1),
                39 => self.move_selection(1),
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for SegmentedControl {
    fn draw(&mut self, context: &mut RenderContext) {
        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the bar, its dividers, its selection and its labels
        // unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style().clone();
        // `segmented_control` reports `WidgetKind::ToggleButton`, whose role is `Primary`; the
        // control itself is not a filled call to action, so the role's primary fill is not used.
        // Its accent serves the selection instead, and the bar derives its own surface below.
        let theme = crate::style::resolved_theme_style("segmented_control");
        // Read as its own lock acquisition and copied out as values, so the guard is dropped
        // before anything else touches the theme.
        let (window_fill, foreground, primary, muted) = {
            let manager = crate::style::theme_manager();
            match manager.current_theme() {
                Some(active) => (
                    active.colors.background,
                    active.colors.foreground,
                    active.colors.primary,
                    active.colors.secondary,
                ),
                None => (
                    Color::rgb(240, 240, 240),
                    Color::BLACK,
                    Color::rgb(33, 150, 243),
                    Color::rgb(158, 158, 158),
                ),
            }
        };

        let ink = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(foreground);
        // The bar: one step from the window fill toward the text colour, so it is a distinct
        // element on a light theme and on a dark one. The filter is on the **resolved** value, not
        // only on the theme's: the active theme is applied to every control before it is drawn, so
        // a control classified as `Surface` already carries the window fill and letting it through
        // unfiltered would make the bar invisible. A caller's own colour still wins.
        let bar_from_theme = window_fill.blend(&ink, 0.08);
        let bar = match style.background_color {
            Some(resolved) if resolved != window_fill => resolved,
            _ => bar_from_theme,
        };
        // The dividers are a fixed step out of the bar so they stay visible whatever the bar is.
        let divider = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| bar.blend(&muted, 0.45));
        // A selected segment is the control's value indicator, so it carries the theme's primary
        // rather than a fixed blue; a hovered one is a lighter step of the same.
        let selected_bg = primary.blend(&bar, 0.55);
        let hovered_bg = primary.blend(&bar, 0.25);

        // ── The bar actually painted ──
        //
        // `rect` is the area the control was *given*; the control's own chrome is one row of
        // segments, `dimensions::SEGMENTED_CONTROL_HEIGHT` tall and centred in that area.
        // Painting the bar across the whole rectangle made a 240x120 census cell a bare
        // 240x120 stadium with no segment division in it — the segments inherited the same
        // height, so there was nothing to divide — and it disagreed with the 32 px size the
        // control reports. The band is the single derivation the paint and the hit test share.
        let band = self.track_band();

        context.fill_rect(band, bar);
        context.draw_rect(band, divider);

        for index in 0..self.items.len() {
            let Some(seg) = self.segment_rect(index) else {
                continue;
            };

            // A hovered segment is a weaker step than the selection, so the two read as one
            // affordance at two weights. The *selected* fill is drawn below as the sliding
            // indicator rather than here, so a segment never paints its own selection.
            if self.hovered_index == Some(index) && self.selected_index != Some(index) {
                context.fill_rect(seg, hovered_bg);
            }

            if index > 0 {
                context.draw_line(
                    Point::new(seg.x, seg.y),
                    Point::new(seg.x, seg.y + seg.height as i32),
                    divider,
                );
            }

            if let Some(item) = self.items.get(index) {
                // The label contrasts with the fill it is painted on — the indicator for the
                // selected segment, the bar for the rest — so it stays legible on either
                // appearance instead of being a fixed dark slate on a dark bar.
                let label_color = if self.selected_index == Some(index) {
                    selected_bg.contrast_color()
                } else {
                    ink
                };
                // The segment's label is centred through the shared primitive: a glyph origin
                // is the top edge of its box, so `seg.y + seg.height / 2` drew the label half
                // a line low rather than on the segment's middle.
                let line = context.text_line(seg, &Font::default());
                // Material's segmented control insets a segment's label by 16 px rather than the
                // 8 the old literal used: at 8 the label of a two-character segment ran into the
                // divider it sits beside.
                context.draw_text(
                    Point::new(seg.x + SEGMENT_LABEL_INSET, line.y),
                    &item.label,
                    &Font::default(),
                    label_color,
                    HorizontalAlignment::Left,
                );
            }
        }

        // ── The selection indicator ──
        //
        // Drawn **after** the segments so it sits over their dividers, and at the *slid* position
        // rather than at `selected_index`: the pill's travel is what tells the reader the value
        // changed, and a pill that teleported would carry no more information than the label colour
        // it already changes. It is interpolated between two segment rectangles rather than between
        // two x offsets, so a segment that changes width (the last one absorbs the remainder) still
        // gives an indicator the right shape at both ends.
        if let (Some(from), Some(to)) = (self.indicator_from(), self.indicator_to()) {
            // `slide` is the fraction travelled, so the pill interpolates between the two segment
            // rectangles. Interpolating *rectangles* rather than two x offsets, so a segment that
            // changes width (the last one absorbs the remainder) still gives an indicator of the
            // right shape at both ends.
            //
            // # The defect this replaces
            //
            // This read `let t = 1.0; // injected: teleport to the target` — a reverse-injection
            // probe that was never removed from the shipping draw path. The consequence was a
            // control whose *model* animated correctly (`tick` advanced the driver,
            // `is_animating` reported true, `indicator_position()` interpolated) while the
            // **pixels teleported**: reading the target fraction pins `lerp` to `to`, so the
            // painted pill jumped to the destination on the first frame of a transition that
            // then spent ~200 ms reporting itself as in-flight. A user saw a jump followed by a
            // motionless wait; an animation test sampling the model saw a smooth slide.
            //
            // That is why the assertion for this belongs on the *paint*, not on
            // `indicator_position`: see `the_indicator_is_painted_mid_slide`.
            let t = self.slide.value();
            let lerp = |a: i32, b: i32| a + ((b - a) as f32 * t) as i32;
            let indicator = Rect::new(
                lerp(from.x, to.x),
                from.y,
                lerp(from.width as i32, to.width as i32).max(1) as u32,
                from.height,
            );
            context.fill_rect(indicator, selected_bg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    fn sample_items() -> Vec<SegmentItem> {
        vec![
            SegmentItem::new("overview", "Overview"),
            SegmentItem::new("details", "Details"),
            SegmentItem::new("history", "History"),
        ]
    }

    #[test]
    fn set_items_selects_first_item() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());

        assert_eq!(control.selected_index(), Some(0));
        assert_eq!(control.selected_id(), Some("overview"));
    }

    #[test]
    fn keyboard_navigation_updates_selection() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());

        control.handle_event(&Event::key_press(39, 0));
        assert_eq!(control.selected_id(), Some("details"));

        control.handle_event(&Event::key_press(39, 0));
        assert_eq!(control.selected_id(), Some("history"));

        control.handle_event(&Event::key_press(37, 0));
        assert_eq!(control.selected_id(), Some("details"));
    }

    #[test]
    fn selection_changed_emits_selected_id() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        control.selection_changed.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        let _ = control.set_selected_index(2);
        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["history".to_string()]);
    }

    #[test]
    fn default_state() {
        let control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        assert!(control.items().is_empty());
        assert_eq!(control.selected_index(), None);
        assert_eq!(control.selected_id(), None);
    }

    #[test]
    fn set_selected_index_get_set() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        control.set_items(vec![
            SegmentItem::new("tab1", "Tab 1"),
            SegmentItem::new("tab2", "Tab 2"),
            SegmentItem::new("tab3", "Tab 3"),
        ]);

        assert_eq!(control.selected_index(), Some(0));
        assert_eq!(control.selected_id(), Some("tab1"));

        assert!(control.set_selected_index(2));
        assert_eq!(control.selected_index(), Some(2));
        assert_eq!(control.selected_id(), Some("tab3"));

        assert!(control.set_selected_index(0));
        assert_eq!(control.selected_index(), Some(0));
        assert_eq!(control.selected_id(), Some("tab1"));
    }

    #[test]
    fn invalid_index_handling() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        control.set_items(vec![SegmentItem::new("a", "A")]);

        // Out of bounds returns false
        assert!(!control.set_selected_index(10));
        assert_eq!(control.selected_index(), Some(0));

        // Valid index returns true
        assert!(control.set_selected_index(0));

        // Move out of bounds clamped
        control.move_selection(10);
        assert_eq!(control.selected_index(), Some(0));

        control.move_selection(-10);
        assert_eq!(control.selected_index(), Some(0));
    }

    #[test]
    fn empty_segments() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        // Navigation on empty should not panic
        control.move_selection(1);
        assert_eq!(control.selected_index(), None);

        // set_selected_index on empty returns false
        assert!(!control.set_selected_index(0));

        // handle event on empty should not panic
        control.handle_event(&Event::key_press(39, 0));
        assert_eq!(control.selected_index(), None);

        control.handle_event(&Event::key_press(37, 0));
        assert_eq!(control.selected_index(), None);
    }

    #[test]
    fn move_selection_previous_next() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 800, 600));
        control.set_items(vec![
            SegmentItem::new("x", "X"),
            SegmentItem::new("y", "Y"),
            SegmentItem::new("z", "Z"),
        ]);

        // Start at 0, move forward
        control.move_selection(1);
        assert_eq!(control.selected_id(), Some("y"));

        control.move_selection(1);
        assert_eq!(control.selected_id(), Some("z"));

        // Move backward
        control.move_selection(-1);
        assert_eq!(control.selected_id(), Some("y"));

        control.move_selection(-1);
        assert_eq!(control.selected_id(), Some("x"));
    }

    /// The bar's height is chrome, not a fraction of the control.
    ///
    /// The defect this pins: the bar and its segments were sized from `rect`, so a 240x120
    /// census cell drew a bare 240x120 stadium whose segments were also 120 tall — there was
    /// nothing left to divide, so the control had no compartment structure at all. The bar's
    /// height is its own, so it is `SEGMENTED_CONTROL_HEIGHT` whenever there is room.
    #[test]
    fn the_bar_keeps_its_own_height_in_any_rectangle() {
        for height in [32u32, 60, 120, 300] {
            let mut control = SegmentedControl::new(Rect::new(0, 0, 240, height));
            control.set_items(sample_items());
            let band = control.track_band();
            assert_eq!(band.height, dimensions::SEGMENTED_CONTROL_HEIGHT, "at {height}");
            for index in 0..control.items().len() {
                let seg = control.segment_rect(index).expect("a laid-out segment");
                assert_eq!(seg.height, dimensions::SEGMENTED_CONTROL_HEIGHT, "at {height}");
            }
        }
    }

    /// The segments tile the bar's width without leaving the control.
    #[test]
    fn the_segments_tile_the_bar() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 240, 120));
        control.set_items(sample_items());
        let band = control.track_band();
        let expected = band.x + band.width as i32;
        let last = control.segment_rect(2).expect("the third segment");
        assert_eq!(last.x + last.width as i32, expected, "the last segment ends at the bar's edge");
    }

    /// Changing the selection slides the indicator through an interior position (§0.3).
    ///
    /// # The defect this pins
    ///
    /// The selected segment's fill was drawn at `selected_index`, so the indicator moved between
    /// two adjacent frames with nothing in between: a three-way switch read as a jump rather than
    /// as a control changing its value. The assertion is the same three-frame shape every animation
    /// in this crate uses — the middle sample must be **strictly between** the two ends — which a
    /// teleporting indicator cannot satisfy.
    #[test]
    fn the_selection_indicator_slides_rather_than_jumping() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 240, 120));
        control.set_items(sample_items());
        // The fractional drawn index: a fresh control *is* at segment 0, so this is exactly 0.0.
        assert_eq!(control.indicator_position(), 0.0, "a fresh control rests on the first segment");
        assert!(!control.is_animating(), "and owes no frames");

        assert!(control.set_selected_index(2));
        assert!(control.is_animating(), "changing the selection owes frames");
        assert_eq!(
            control.indicator_position(),
            0.0,
            "and it starts from the segment the pill was already on"
        );

        assert!(control.tick(60), "still moving after one step");
        let mid = control.indicator_position();
        assert!(
            mid > 0.0 && mid < 2.0,
            "the indicator must take an interior position, not jump to either end (got {mid})"
        );
        while control.tick(60) {}
        assert_eq!(control.indicator_position(), 2.0, "and settle on the selected segment");
        assert!(!control.is_animating(), "a settled control owes no more frames");
    }

    /// The pill's **painted** rect takes an interior position mid-slide, not just the model.
    ///
    /// # The defect this pins
    ///
    /// `draw` read `let t = 1.0; // injected: teleport to the target` — a reverse-injection probe
    /// left in the shipping draw path. Everything above this test still passed, because it samples
    /// `indicator_position()`, which the *driver* feeds correctly: `tick` advanced, `is_animating`
    /// reported true, and the model interpolated. Only the pixels were wrong — `t = 1.0` pins the
    /// `lerp` to `to`, so the painted pill sat on the destination from the first frame of a
    /// transition that then spend ~200 ms claiming to be in flight.
    ///
    /// # Why this reads the SVG
    ///
    /// The quantity that was broken is the rectangle handed to `fill_rect`, so the assertion is on
    /// the rendered picture. Asserting `indicator_position()` is the mistake this test exists to
    /// avoid: it is the same value the old test checked, and it was never wrong.
    #[test]
    fn the_indicator_is_painted_mid_slide() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 240, 120));
        control.set_items(sample_items());

        // The pill at rest, from the picture rather than the model.
        let settled = painted_indicator_x(&mut control);

        assert!(control.set_selected_index(2));
        assert!(control.tick(60), "still moving after one step");
        let mid_slide = painted_indicator_x(&mut control);
        assert!(
            mid_slide > settled,
            "the painted pill must have moved off its resting segment ({settled}) while the \
             slide is in flight, but it is still painted at {mid_slide} — a `t` pinned to 1.0 \
             draws the destination immediately and leaves the control reporting frames it is not \
             using"
        );

        while control.tick(60) {}
        let arrived = painted_indicator_x(&mut control);
        assert!(
            arrived > mid_slide,
            "and it must finish further along than it was mid-slide ({mid_slide} -> {arrived})"
        );
    }

    /// The x of the filled pill in the control's rendered SVG.
    ///
    /// The pill is the one filled `<rect>` that is neither the frame-spanning track nor a glyph
    /// run, so it is found by geometry: it is inset from the control and taller than a hairline.
    fn painted_indicator_x(control: &mut SegmentedControl) -> i32 {
        let svg = crate::widget::svg::render_widget_to_svg(control, Rect::new(0, 0, 240, 120));
        let mut found = None;
        for line in svg.lines() {
            let trimmed = line.trim();
            if !trimmed.starts_with("<rect") || !trimmed.contains("fill=") {
                continue;
            }
            let numbers: alloc::vec::Vec<i32> =
                trimmed.split('"').filter_map(|part| part.parse::<i32>().ok()).collect();
            // `x y width height`: the track spans the control, the pill does not; both are the
            // band's height, so a glyph run (1x1 cells) is excluded by the height.
            if numbers.len() >= 4 && numbers[2] < 240 && numbers[3] > 4 {
                // Keep the last: the indicator is painted **over** the segments, so an earlier
                // inset rect is a hover fill, and returning the first one reads the wrong box.
                // (That mistake made this probe report "still at 0" for a pill that had moved.)
                found = Some(numbers[0]);
            }
        }
        found.unwrap_or_else(|| panic!("no pill rect found in:\n{svg}"))
    }

    /// A slide interrupted by a second selection stays continuous.
    ///
    /// The user clicks segment 1, then segment 2 while the pill is still travelling. The movement
    /// must resume from wherever the pill is — a slide that restarted from segment 0 would visibly
    /// jump backwards first, which is the one thing an indicator exists to avoid.
    #[test]
    fn a_second_selection_mid_slide_resumes_from_where_the_pill_is() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 240, 120));
        control.set_items(sample_items());

        assert!(control.set_selected_index(1));
        assert!(control.tick(60));
        let midway = control.indicator_position();
        assert!(midway > 0.0 && midway < 1.0, "part-way to the first target (got {midway})");

        assert!(control.set_selected_index(2));
        let after = control.indicator_position();
        assert!(
            after >= midway && after < 1.0,
            "the new slide must resume at or past where the pill was, never behind it (got {after})"
        );

        while control.tick(60) {}
        assert_eq!(control.indicator_position(), 2.0, "and still reach the new selection");
    }

    /// A new item set resets the indicator instead of sliding it.
    ///
    /// A reset is not a selection change: the control the caller just rebuilt has no previous
    /// selection to travel from, so animating out of the old one would depict a gesture nobody made.
    #[test]
    fn a_new_item_set_resets_the_indicator() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 240, 120));
        control.set_items(sample_items());
        assert!(control.set_selected_index(2));
        assert!(control.is_animating(), "the old set left a slide in flight");

        control.set_items(sample_items());
        assert_eq!(control.indicator_position(), 0.0, "a rebuilt set starts at the first segment");
        assert!(!control.is_animating(), "with nothing in flight");
    }

    /// The label is inset by the segment's own padding, not by a mark-hugging 8.
    ///
    /// A label that sits too close to its divider reads as part of the divider. The assertion is a
    /// **relation**, not a threshold against the constant: the first glyph of a segment's label
    /// must sit at least the inset in from that segment's edge, *and* it must be further in than the
    /// bar's own edge by more than a glyph bearing. A threshold alone is not enough — at inset `0`
    /// the glyph's own left bearing already puts it at x≈2, which satisfies any `>= first.x + 0`.
    #[test]
    fn the_label_takes_the_segment_padding() {
        use crate::widget::svg::render_to_svg;

        let mut control = SegmentedControl::new(Rect::new(0, 0, 240, 120));
        control.set_items(sample_items());
        let first = control.segment_rect(0).expect("a laid-out first segment");
        let svg = render_to_svg(&mut control);

        // The first segment's **label ink**, located by the segment it lies in rather than by the
        // subpath spelling of a `d` string.
        //
        // The first form read `d="M{x} {y}..."` by taking leading *integer* digits, which is the
        // bitmap face's grammar only: an outline writes `d="M23.44 64.25L..."`, so the reader took
        // `23` as `x` and then refused the run because the next character was `.` rather than a
        // space — no label was ever found. Reading the run's union ink box through the crate's own
        // reader handles both grammars, and its left edge *is* the leftmost glyph column, which is
        // the quantity the inset is measured from. The bar and the indicator are `<rect>`s, so the
        // only `<path>` runs are the labels and the band test selects the first one.
        let label = crate::widget::svg::text_ink_boxes(&svg)
            .into_iter()
            .find(|(l, _, r, _)| {
                let centre = (l + r) / 2;
                centre >= first.x && centre < first.x + first.width as i32
            })
            .expect("the first segment's label was painted");
        let label_start = label.0;

        let painted_inset = label_start - first.x;
        assert!(
            painted_inset >= SEGMENT_LABEL_INSET,
            "the label starts {painted_inset} px in; the segment's padding is {SEGMENT_LABEL_INSET}"
        );
        // The padding is a *padding*, not the glyph's own bearing: an uninset label would land
        // within a couple of pixels of the edge, which is the shape this pins.
        assert!(
            painted_inset > 4,
            "a label {painted_inset} px from its edge is hugging it, which is the defect"
        );
    }
}
