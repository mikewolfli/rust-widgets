// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SegmentedControl widget.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The horizontal inset a segment's label takes from its own edge: 16.
///
/// Material's segmented control (`ButtonSegment`'s `padding`) insets a segment's label by 16 px. It
/// was `8`, which put a short label close enough to its divider that the two read as one mark.
const SEGMENT_LABEL_INSET: i32 = 16;

/// Horizontal room reserved for a segment's leading icon, including its gap: 18.
const SEGMENT_ICON_SLOT: i32 = 18;

/// The narrowest a content-fitted segment may become.
const DEFAULT_MIN_SEGMENT_WIDTH: u32 = 56;

/// Single segment entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentItem {
    /// Stable item id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Optional leading icon.
    pub icon: Option<IconSource>,
}

impl SegmentItem {
    /// Creates one segment item.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), icon: None }
    }

    /// Sets the segment's leading icon.
    pub fn with_icon(mut self, icon: IconSource) -> Self {
        self.icon = Some(icon);
        self
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
    /// Whether segment widths follow their labels instead of splitting the track evenly.
    size_to_fit: bool,
    /// The narrowest a content-fitted segment may become.
    min_segment_width: u32,
    /// Whether several segments may be selected at once.
    multi_select: bool,
    /// Per-segment enabled flags, index-aligned with `items`. A missing entry is enabled.
    disabled: Vec<bool>,
    /// The selected indices in multi-select mode. Ignored in single-select mode, where
    /// `selected_index` is the whole answer.
    selected_set: Vec<usize>,
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
            size_to_fit: false,
            min_segment_width: DEFAULT_MIN_SEGMENT_WIDTH,
            multi_select: false,
            disabled: Vec::new(),
            selected_set: Vec::new(),
            selection_changed: Signal1::new(),
        }
    }

    /// Replaces all segment items.
    pub fn set_items(&mut self, items: Vec<SegmentItem>) {
        self.items = items;
        self.disabled = vec![false; self.items.len()];
        self.selected_set.clear();
        self.selected_index = if self.items.is_empty() { None } else { Some(0) };
        self.hovered_index = self.selected_index;
        // A new item set is a *reset*, not a selection change: the indicator starts on the first
        // segment rather than sliding there from wherever the old set left it.
        self.slide.jump_to(1.0);
        self.slide_from = 0;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Appends one segment, returning its index.
    ///
    /// `set_items` replaces the whole list, so adding a segment meant cloning every existing item
    /// and resetting the selection — which a caller that grows its option set one entry at a time
    /// could not avoid.
    pub fn add_item(&mut self, item: SegmentItem) -> usize {
        let index = self.items.len();
        self.items.push(item);
        self.disabled.push(false);
        if self.selected_index.is_none() {
            self.selected_index = Some(index);
            self.slide_from = index;
            self.slide.jump_to(1.0);
        }
        self.base.request_layout();
        self.base.request_redraw();
        index
    }

    /// Removes the segment at `index`, returning it when it was present.
    ///
    /// The selection is repaired rather than dropped blindly: removing a segment above the selected
    /// one shifts the selection down, removing the selected one selects the nearest remaining
    /// segment, and removing the last segment leaves no selection.
    pub fn remove_item(&mut self, index: usize) -> Option<SegmentItem> {
        if index >= self.items.len() {
            return None;
        }
        let removed = self.items.remove(index);
        if index < self.disabled.len() {
            self.disabled.remove(index);
        }
        self.selected_set.retain(|selected| *selected != index);
        for selected in &mut self.selected_set {
            if *selected > index {
                *selected -= 1;
            }
        }
        self.selected_index = match self.selected_index {
            Some(selected) if selected == index => {
                if self.items.is_empty() {
                    None
                } else {
                    Some(index.min(self.items.len() - 1))
                }
            }
            Some(selected) if selected > index => Some(selected - 1),
            other => other,
        };
        self.slide_from = self.selected_index.unwrap_or(0);
        self.slide.jump_to(1.0);
        self.base.request_layout();
        self.base.request_redraw();
        Some(removed)
    }

    /// Enables or disables the segment at `index`, returning `false` when it does not exist.
    pub fn set_item_enabled(&mut self, index: usize, enabled: bool) -> bool {
        if index >= self.items.len() {
            return false;
        }
        if self.disabled.len() <= index {
            self.disabled.resize(index + 1, false);
        }
        self.disabled[index] = !enabled;
        if !enabled && self.selected_index == Some(index) {
            // A disabled segment cannot remain the control's value: the value would be one the
            // user can neither see as chosen nor change away from by clicking it again.
            let next = (0..self.items.len()).find(|candidate| self.item_enabled(*candidate));
            self.selected_index = next;
        }
        self.base.request_redraw();
        true
    }

    /// Whether the segment at `index` accepts input.
    pub fn item_enabled(&self, index: usize) -> bool {
        index < self.items.len() && !self.disabled.get(index).copied().unwrap_or(false)
    }

    /// The selected segment indices.
    pub fn selected_indices(&self) -> Vec<usize> {
        if self.multi_select {
            let mut selected = self.selected_set.clone();
            selected.sort_unstable();
            selected
        } else {
            self.selected_index().into_iter().collect()
        }
    }

    /// Whether the segment at `index` is selected.
    pub fn is_selected(&self, index: usize) -> bool {
        if self.multi_select {
            self.selected_set.contains(&index)
        } else {
            self.selected_index() == Some(index)
        }
    }

    /// Whether widths follow the labels.
    pub fn size_to_fit(&self) -> bool {
        self.size_to_fit
    }

    /// Sets whether segment widths follow their labels instead of splitting the track evenly.
    pub fn set_size_to_fit(&mut self, size_to_fit: bool) {
        if self.size_to_fit == size_to_fit {
            return;
        }
        self.size_to_fit = size_to_fit;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The narrowest a content-fitted segment may become.
    pub fn min_segment_width(&self) -> u32 {
        self.min_segment_width
    }

    /// Sets the narrowest a content-fitted segment may become (floored at 1).
    pub fn set_min_segment_width(&mut self, width: u32) {
        let width = width.max(1);
        if self.min_segment_width == width {
            return;
        }
        self.min_segment_width = width;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Whether several segments may be selected at once.
    pub fn multi_select(&self) -> bool {
        self.multi_select
    }

    /// Enables or disables multi-selection.
    ///
    /// Turning it on keeps the current single selection as the first member of the set; turning it
    /// off collapses a multi-selection to its most recently clicked segment.
    pub fn set_multi_select(&mut self, multi_select: bool) {
        if self.multi_select == multi_select {
            return;
        }
        self.multi_select = multi_select;
        if multi_select {
            self.selected_set = self.selected_index.into_iter().collect();
        } else {
            self.selected_set.clear();
        }
        self.base.request_redraw();
    }

    /// Toggles the segment at `index` in multi-select mode.
    ///
    /// Returns the segment's new selected state; `false` when the index is not a segment, when it is
    /// disabled, or when the control is in single-select mode (where there is nothing to toggle).
    pub fn toggle_selected(&mut self, index: usize) -> bool {
        if !self.multi_select || !self.item_enabled(index) {
            return false;
        }
        let now_selected =
            if let Some(position) = self.selected_set.iter().position(|i| *i == index) {
                self.selected_set.remove(position);
                false
            } else {
                self.selected_set.push(index);
                self.selected_set.sort_unstable();
                true
            };
        // The sliding pill belongs to single-select mode; in multi-select the selected segments are
        // filled statically, so the anchor is kept current instead of animated.
        self.selected_index = Some(index);
        self.slide_from = index;
        self.slide.jump_to(1.0);
        if let Some(item) = self.items.get(index) {
            self.selection_changed.emit(item.id.clone());
        }
        self.base.request_redraw();
        now_selected
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
        if index >= self.items.len() || !self.item_enabled(index) {
            return false;
        }
        if self.selected_index == Some(index) {
            return true;
        }
        // A programmatic single selection collapses a multi-selection to the one requested: the
        // call asks for *the* selected segment, and leaving the old group in place would make
        // `selected_indices` disagree with the index this method just accepted.
        if self.multi_select {
            self.selected_set = vec![index];
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
    ///
    /// A disabled segment is stepped over: the selection moves to the next segment the user can
    /// actually reach, and stays put when there is none.
    pub fn move_selection(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.selected_index = None;
            return;
        }
        let start = self.selected_index.unwrap_or(0) as isize;
        let last = self.items.len() as isize - 1;
        let step = if delta < 0 { -1 } else { 1 };
        let mut candidate = start + step;
        while candidate >= 0 && candidate <= last {
            if self.item_enabled(candidate as usize) {
                let _ = self.set_selected_index(candidate as usize);
                return;
            }
            candidate += step;
        }
    }

    /// The width each segment's own content asks for.
    fn desired_segment_widths(&self) -> Vec<u32> {
        let font = effective_font(self.base.style());
        self.items
            .iter()
            .map(|item| {
                let text = estimate_text_width(&item.label, font, 1.0);
                let icon = if item.icon.is_some() { SEGMENT_ICON_SLOT as u32 } else { 0 };
                text.saturating_add((SEGMENT_LABEL_INSET as u32) * 2)
                    .saturating_add(icon)
                    .max(self.min_segment_width)
            })
            .collect()
    }

    /// The painted width of every segment within `band_width`.
    fn segment_widths(&self, band_width: u32) -> Vec<u32> {
        let count = self.items.len();
        if count == 0 {
            return Vec::new();
        }
        if !self.size_to_fit {
            // The historical layout: equal shares, with the last segment absorbing the remainder so
            // the bar has no 1 px seam at its right edge.
            let width = (band_width / count as u32).max(1);
            let mut widths = vec![width; count];
            if let Some(last) = widths.last_mut() {
                *last = band_width.saturating_sub(width * (count as u32 - 1)).max(1);
            }
            return widths;
        }
        let desired = self.desired_segment_widths();
        let total: u32 = desired.iter().sum();
        if total <= band_width {
            // Content is smaller than the track: the extra room stays empty rather than being
            // handed to a short label, which is the whole point of fitting to content.
            return desired;
        }
        // Content overflows: shrink proportionally, never below one pixel, and let the last segment
        // take the remainder.
        let mut widths = Vec::with_capacity(count);
        let mut consumed = 0u32;
        for (index, width) in desired.iter().enumerate() {
            if index + 1 == count {
                widths.push(band_width.saturating_sub(consumed).max(1));
            } else {
                let scaled = ((u64::from(*width) * u64::from(band_width)) / u64::from(total.max(1)))
                    .max(1) as u32;
                consumed = consumed.saturating_add(scaled);
                widths.push(scaled);
            }
        }
        widths
    }

    fn segment_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.items.len() || self.items.is_empty() {
            return None;
        }
        let band = self.track_band();
        let widths = self.segment_widths(band.width);
        let width = widths.get(index).copied()?;
        let x = band.x + widths.iter().take(index).map(|w| *w as i32).sum::<i32>();
        Some(Rect::new(x, band.y, width, band.height))
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
        let full =
            ControlMetrics::full_width_band(self.geometry(), dimensions::SEGMENTED_CONTROL_HEIGHT);
        if !self.size_to_fit || self.items.is_empty() {
            return full;
        }
        // A content-fitted control is as wide as its segments need, never wider than the room it
        // was given — that is what makes a two-word segment narrow and a long one wide.
        let total: u32 = self.desired_segment_widths().iter().sum();
        Rect::new(full.x, full.y, total.min(full.width), full.height)
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
                Some(EventSignalRef::mapped("selection_changed", &self.selection_changed, |v| {
                    CapabilityValue::String(v.clone())
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
            "size_to_fit" => Ok(CapabilityValue::Bool(self.size_to_fit())),
            "min_segment_width" => Ok(CapabilityValue::UInt(self.min_segment_width() as u64)),
            "multi_select" => Ok(CapabilityValue::Bool(self.multi_select())),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            // The selection is derived state: a press or `move_selection` decides it, and the
            // schema publishes both names as read-only. The property route therefore refuses a
            // write outright instead of accepting a value it would have to reject later by type,
            // which is what the declaration gate reads as "the control thinks it is writable".
            // Callers that want to move the selection use the `move_selection` command or the
            // typed `set_selected_index` / `set_selected_id` API.
            "selected_index" | "selected_id" => {
                let _ = value;
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "size_to_fit" => {
                let size_to_fit = match value {
                    CapabilityValue::Bool(size_to_fit) => size_to_fit,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_size_to_fit(size_to_fit);
                Ok(())
            }
            "min_segment_width" => {
                let width = match value {
                    CapabilityValue::UInt(width) => {
                        u32::try_from(width).map_err(|_| CapabilityAccessError::TypeMismatch)?
                    }
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_min_segment_width(width);
                Ok(())
            }
            "multi_select" => {
                let multi_select = match value {
                    CapabilityValue::Bool(multi_select) => multi_select,
                    _ => return Err(CapabilityAccessError::TypeMismatch),
                };
                self.set_multi_select(multi_select);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "item_count",
            "selected_index",
            "selected_id",
            "size_to_fit",
            "min_segment_width",
            "multi_select",
            BASE_PROPERTY_NAMES
        ]
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
                let hovered = self.hit_index(*pos).filter(|index| self.item_enabled(*index));
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
                    if !self.item_enabled(index) {
                        return;
                    }
                    if self.multi_select {
                        let _ = self.toggle_selected(index);
                    } else {
                        let _ = self.set_selected_index(index);
                    }
                }
            }
            Event::KeyPress { key, modifiers } => {
                let pressed = crate::shortcut::Key::from_key_code(*key);
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                match pressed {
                    Some(crate::shortcut::Key::Left) => self.move_selection(-1),
                    Some(crate::shortcut::Key::Right) => self.move_selection(1),
                    Some(crate::shortcut::Key::Home) => {
                        if let Some(first) =
                            (0..self.items.len()).find(|index| self.item_enabled(*index))
                        {
                            let _ = self.set_selected_index(first);
                        }
                    }
                    Some(crate::shortcut::Key::End) if !self.items.is_empty() => {
                        if let Some(last) =
                            (0..self.items.len()).rev().find(|index| self.item_enabled(*index))
                        {
                            let _ = self.set_selected_index(last);
                        }
                    }
                    Some(crate::shortcut::Key::Up)
                        if modifiers.contains(crate::shortcut::Modifiers::PRIMARY) =>
                    {
                        if let Some(first) =
                            (0..self.items.len()).find(|index| self.item_enabled(*index))
                        {
                            let _ = self.set_selected_index(first);
                        }
                    }
                    Some(crate::shortcut::Key::Down)
                        if modifiers.contains(crate::shortcut::Modifiers::PRIMARY)
                            && !self.items.is_empty() =>
                    {
                        if let Some(last) =
                            (0..self.items.len()).rev().find(|index| self.item_enabled(*index))
                        {
                            let _ = self.set_selected_index(last);
                        }
                    }
                    _ => {}
                }
            }
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
        let style = self.base.style();
        // `segmented_control` reports `WidgetKind::ToggleButton`, whose role is `Primary`; the
        // control itself is not a filled call to action, so the role's primary fill is not used.
        // Its accent serves the selection instead, and the bar derives its own surface below.
        let theme = crate::style::resolved_theme_style_shared("segmented_control");
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
            // indicator rather than here, so a segment never paints its own selection. A disabled
            // segment never takes the hover weight: it cannot be pressed, so highlighting it would
            // promise an action that does not exist.
            if self.hovered_index == Some(index)
                && !self.is_selected(index)
                && self.item_enabled(index)
            {
                context.fill_rect(seg, hovered_bg);
            }

            if index > 0 {
                context.draw_line(
                    Point::new(seg.x, seg.y),
                    Point::new(seg.x, seg.y + seg.height as i32),
                    divider,
                );
            }
        }

        // ── The selection indicator ──
        //
        // Drawn **after** the bar and dividers so it sits over them, and at the *slid* position
        // rather than at `selected_index`: the pill's travel is what tells the reader the value
        // changed, and a pill that teleported would carry no more information than the label colour
        // it already changes. It is interpolated between two segment rectangles rather than between
        // two x offsets, so a segment that changes width (the last one absorbs the remainder) still
        // gives an indicator the right shape at both ends.
        //
        // # Why the labels come after this, not before
        //
        // The two used to be interleaved per segment, so the pill was painted **over** the
        // selected segment's label. The label was still drawn — with `contrast_color()` picked
        // for the pill — but the opaque fill covered it, and the selected segment rendered as a
        // blank blue slab. A selected control whose label disappears is not a colour problem; it
        // is a z-order problem, and the fix is one pass for fills and a second for text.
        if self.multi_select {
            // Multi-selection has no single destination for a sliding pill, so each selected
            // segment is filled statically. The slide is single-select's way of saying "the value
            // moved"; a set has no one place to move to.
            for index in 0..self.items.len() {
                if !self.is_selected(index) {
                    continue;
                }
                if let Some(seg) = self.segment_rect(index) {
                    context.fill_rect(seg, selected_bg);
                }
            }
        } else if let (Some(from), Some(to)) = (self.indicator_from(), self.indicator_to()) {
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

        // ── Segment labels ──
        for index in 0..self.items.len() {
            let Some(seg) = self.segment_rect(index) else {
                continue;
            };
            if let Some(item) = self.items.get(index) {
                // The label contrasts with the fill it is painted on — the indicator for the
                // selected segment, the bar for the rest — so it stays legible on either
                // appearance instead of being a fixed dark slate on a dark bar.
                let label_color = if self.is_selected(index) {
                    selected_bg.contrast_color()
                } else if !self.item_enabled(index) {
                    // A disabled segment reads as unavailable rather than as a different value.
                    ink.blend(&bar, 0.55)
                } else {
                    ink
                };
                // The segment's label is centred through the shared primitive: a glyph origin
                // is the top edge of its box, so `seg.y + seg.height / 2` drew the label half
                // a line low rather than on the segment's middle.
                // The **effective font** — the resolved theme/caller font — for the segment labels
                // (D09-STYLE-01).
                let line = context.text_line(seg, effective_font(style));
                // Material's segmented control insets a segment's label by 16 px rather than the
                // 8 the old literal used: at 8 the label of a two-character segment ran into the
                // divider it sits beside.
                context.draw_text(
                    Point::new(
                        seg.x
                            + SEGMENT_LABEL_INSET
                            + if item.icon.is_some() { SEGMENT_ICON_SLOT } else { 0 },
                        line.y,
                    ),
                    &item.label,
                    effective_font(style),
                    label_color,
                    HorizontalAlignment::Left,
                );
                if let Some(icon) = item.icon {
                    let side = 12u32.min(seg.height.saturating_sub(4)).max(1);
                    let icon_rect = Rect::new(
                        seg.x + SEGMENT_LABEL_INSET,
                        seg.y + (seg.height.saturating_sub(side) / 2) as i32,
                        side,
                        side,
                    );
                    if let Err(error) =
                        crate::widget::paint_icon(context, icon.into(), icon_rect, label_color)
                    {
                        log::warn!("segment '{}' icon failed: {error:?}", item.id);
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Color, Font, Size};
    use crate::render::{
        PaintBackend, RenderCommand, RenderContext, ShapedText, SoftwarePaintBackend,
        SoftwareRenderConfig, TextMetrics,
    };
    use std::sync::{Arc, Mutex};

    /// A backend that records every command in draw order.
    ///
    /// Text and fills are separate commands, so "the label is painted over the pill" is
    /// an ordered fact a test can read without a pixel comparison.
    struct Recorder {
        commands: Vec<RenderCommand>,
        inner: SoftwarePaintBackend,
    }

    impl Recorder {
        fn new(size: Size) -> Self {
            Self { commands: Vec::new(), inner: SoftwarePaintBackend::new(size, 1.0) }
        }
    }

    impl PaintBackend for Recorder {
        fn begin_frame(&mut self, clear: Color) {
            self.inner.begin_frame(clear);
        }
        fn end_frame(&mut self) {
            self.inner.end_frame();
        }
        fn execute_command(&mut self, command: &RenderCommand) {
            self.commands.push(command.clone());
            self.inner.execute_command(command);
        }
        fn size(&self) -> Size {
            self.inner.size()
        }
        fn set_size(&mut self, size: Size) {
            self.inner.set_size(size);
        }
        fn dpi_scale(&self) -> f32 {
            self.inner.dpi_scale()
        }
        fn set_dpi_scale(&mut self, scale: f32) {
            self.inner.set_dpi_scale(scale);
        }
        fn measure_text(&self, text: &str, font: &Font) -> TextMetrics {
            self.inner.measure_text(text, font)
        }
        fn shape_text(&self, text: &str, font: &Font) -> crate::compat::Rc<ShapedText> {
            self.inner.shape_text(text, font)
        }
        fn frame_rgba(&self) -> &[u8] {
            self.inner.frame_rgba()
        }
        fn apply_render_config(&mut self, config: SoftwareRenderConfig) {
            self.inner.apply_render_config(config);
        }
    }

    fn sample_items() -> Vec<SegmentItem> {
        vec![
            SegmentItem::new("overview", "Overview"),
            SegmentItem::new("details", "Details"),
            SegmentItem::new("history", "History"),
        ]
    }

    /// The selected segment's label is painted **after** the selection pill.
    ///
    /// The pill is an opaque fill drawn in the same loop as the labels and after each one,
    /// so it covered the selected label completely: the control showed a blank blue slab
    /// where "Overview" should be. Presence of the text command is not the property that
    /// matters — order is.
    #[test]
    fn the_selected_label_is_painted_after_the_indicator() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());
        assert_eq!(control.selected_index(), Some(0));
        let segment = control.segment_rect(0).expect("the first segment exists");

        let mut recorder = Recorder::new(Size::new(300, 30));
        recorder.begin_frame(Color::WHITE);
        {
            let mut context = RenderContext::new(&mut recorder);
            control.draw(&mut context);
        }
        recorder.end_frame();

        let mut indicator_at = None;
        let mut label_at = None;
        for (index, command) in recorder.commands.iter().enumerate() {
            match command {
                RenderCommand::FillRect { rect, .. } if *rect == segment => {
                    indicator_at = Some(index);
                }
                RenderCommand::DrawText { text, .. } if text.as_str() == "Overview" => {
                    label_at = Some(index);
                }
                _ => {}
            }
        }
        let label_at = label_at.expect("the selected segment's label is drawn");
        let indicator_at = indicator_at.expect("the selection pill is drawn");
        assert!(
            label_at > indicator_at,
            "the label must come after the pill (label at {label_at}, pill at {indicator_at})"
        );
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

    #[test]
    fn add_and_remove_items_keep_the_selection_valid() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        assert_eq!(control.add_item(SegmentItem::new("a", "A")), 0);
        assert_eq!(control.selected_id(), Some("a"), "the first added item is selected");
        assert_eq!(control.add_item(SegmentItem::new("b", "B")), 1);
        assert!(control.remove_item(5).is_none());

        assert!(control.set_selected_index(1));
        let removed = control.remove_item(0).expect("item a exists");
        assert_eq!(removed.id, "a");
        assert_eq!(control.selected_id(), Some("b"), "the selection follows its item down");

        control.remove_item(0);
        assert_eq!(control.selected_index(), None, "removing the last item clears selection");
    }

    #[test]
    fn a_disabled_segment_cannot_be_selected() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());
        assert!(control.set_item_enabled(1, false));
        assert!(!control.item_enabled(1));

        // A programmatic selection refuses it, and a click on it changes nothing.
        assert!(!control.set_selected_index(1));
        let segment = control.segment_rect(1).expect("segment 1 exists");
        let y = segment.y + segment.height as i32 / 2;
        control.handle_event(&Event::mouse_press(segment.x + 2, y, 1));
        assert_eq!(control.selected_index(), Some(0));

        // Keyboard navigation steps over it to the next enabled segment.
        control.handle_event(&Event::key_press(39, 0));
        assert_eq!(control.selected_index(), Some(2));

        // Disabling the selected segment moves the selection to a usable one.
        control.set_selected_index(0);
        assert!(control.set_item_enabled(0, false));
        assert_eq!(control.selected_index(), Some(2));
    }

    #[test]
    fn multi_select_toggles_and_tracks_a_set() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 300, 30));
        control.set_items(sample_items());
        control.set_multi_select(true);
        assert!(control.multi_select());

        // The initial single selection becomes the first member of the set.
        assert_eq!(control.selected_indices(), vec![0]);
        assert!(control.toggle_selected(2));
        assert_eq!(control.selected_indices(), vec![0, 2]);
        assert!(!control.toggle_selected(0));
        assert_eq!(control.selected_indices(), vec![2]);

        // A single-select API call collapses the group to the one requested.
        assert!(control.set_selected_index(1));
        assert_eq!(control.selected_indices(), vec![1]);

        // Turning multi-select off leaves the current single selection as the value.
        control.set_multi_select(false);
        assert_eq!(control.selected_indices(), vec![1]);
    }

    #[test]
    fn size_to_fit_gives_each_segment_its_label_width() {
        let mut control = SegmentedControl::new(Rect::new(0, 0, 400, 30));
        control.set_items(vec![
            SegmentItem::new("short", "A"),
            SegmentItem::new("long", "A much longer caption"),
        ]);
        // Equal division: the short label wastes half the track.
        let short = control.segment_rect(0).expect("segment 0").width;
        let long = control.segment_rect(1).expect("segment 1").width;
        assert_eq!(short, long);

        control.set_size_to_fit(true);
        assert!(control.size_to_fit());
        let short = control.segment_rect(0).expect("segment 0").width;
        let long = control.segment_rect(1).expect("segment 1").width;
        assert!(
            long > short,
            "a long caption must get a wider segment: short {short}, long {long}"
        );
        assert!(short >= control.min_segment_width());

        // The property is written through the same setter.
        control.set("min_segment_width", CapabilityValue::UInt(90)).expect("writable");
        assert_eq!(control.min_segment_width(), 90);
        assert!(control.segment_rect(0).expect("segment 0").width >= 90);
    }
}
