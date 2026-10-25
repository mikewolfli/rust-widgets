// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! SegmentedButton widget — a Material 3-style segmented button group.
//!
//! The SegmentedButton widget displays a horizontal row of segments where the user
//! can select one (single-select) or multiple (multi-select) options. Each segment
//! displays a text label and an optional icon. The active/highlighted segment uses
//! a filled background to indicate selection.

use crate::compat::Cow;
use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::{expect_bool, expect_string, expect_usize};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, role_font, ControlMetrics};
use crate::widget::text_utils::elide_to_width;
use crate::widget::{BaseWidget, Draw, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// Square side of a segment's leading icon slot.
const SEGMENT_ICON_SIDE: u32 = 14;
/// Gap between that slot and the caption.
const SEGMENT_ICON_GAP: u32 = 4;

/// Process-wide ownership of a mutually exclusive segmented-button group.
///
/// A group is identified by a caller-chosen string; the entry records which control last
/// selected a segment and a generation counter so every member can tell whether its local
/// selection is still the authoritative one. A global map is what makes two controls that
/// know nothing about each other able to share one selection without a host-owned registry.
type SegmentedGroupEntry = (u64, u64, Option<usize>);
type SegmentedGroupTable =
    std::sync::Mutex<std::collections::BTreeMap<String, SegmentedGroupEntry>>;

fn segmented_groups() -> &'static SegmentedGroupTable {
    static GROUPS: std::sync::OnceLock<SegmentedGroupTable> = std::sync::OnceLock::new();
    GROUPS.get_or_init(|| std::sync::Mutex::new(std::collections::BTreeMap::new()))
}

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
    /// Optional hover hint, shown when the pointer is over the segment (MISC-15).
    pub tooltip: Option<String>,
}

impl Segment {
    /// Creates a new segment with the given label text.
    pub fn new(text: &str) -> Self {
        Self {
            id: text.to_string(),
            text: text.to_string(),
            icon: None,
            enabled: true,
            tooltip: None,
        }
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

    /// Sets the segment's hover hint.
    ///
    /// A caption that the segment has to elide is the case a hint exists for, and there was no
    /// way to give one (MISC-15).
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// The segment's hover hint, if one is set.
    pub fn tooltip(&self) -> Option<&str> {
        self.tooltip.as_deref()
    }
}

/// SegmentedButton widget — a horizontal group of selectable segments.
///
/// Supports single-select (default) and **multi-select** modes. In single-select mode, clicking a
/// segment selects it and deselects all others; in multi-select mode each click toggles that
/// segment and the others are left alone.
///
/// # Why the selection is a set, not one slot
///
/// `allows_multiple` shipped as a flag over a single `selected_index: Option<usize>`, so
/// "multi-select" could only ever hold **one** selection — the flag promised a behaviour the state
/// could not represent (the doc claimed multi-select; clicking a second segment silently dropped
/// the first). The selection is therefore a sorted `Vec<usize>`, which is also what `ListBox` uses
/// for its multi-select. [`Self::selected_index`] keeps reporting the *primary* selection so the
/// single-select API and its callers are unchanged.
pub struct SegmentedButton {
    base: BaseWidget,
    segments: Vec<Segment>,
    /// Every selected segment index, ascending and deduplicated.
    ///
    /// In single-select mode it holds at most one entry, which is what makes the two modes one
    /// model rather than two (the invariant is enforced by [`Self::set_selected_index`] and the
    /// click handler).
    selected: Vec<usize>,
    allows_multiple: bool,
    /// Mutually exclusive group id; controls with the same id share one selection.
    group_id: Option<String>,
    /// The group generation this control last observed.
    group_generation: u64,
    /// Emitted when the selected segment changes. Payload is the newly selected index.
    pub current_changed: Signal1<usize>,
    /// The segment the pointer is over, for the hover hint (MISC-15).
    hovered_segment: Option<usize>,
    /// The segment the keyboard is on (MISC-14).
    focus_index: Option<usize>,
    /// Per-segment composed label text and its measurement, refreshed only when the label's inputs
    /// change (K-P15).
    ///
    /// The draw path cloned every caption, formatted the icon-name form again and measured it on
    /// every frame, then had the fitting primitive measure it once more.
    segment_labels: Vec<Option<SegmentLabelCache>>,
    /// The host-side hover/delay/show pipeline for segment hints (MISC-15).
    tooltips: crate::widget::companions::TooltipManager,
    /// The derived hover id currently registered with the tooltip pipeline.
    tooltip_source: Option<crate::core::ObjectId>,
    /// Monotonic clock fed to the tooltip pipeline from [`Widget::tick`].
    tooltip_now_ms: u64,
}

/// One segment's composed caption and the measurement of it.
///
/// `text` is the caption as painted (the icon-name prefix included when the icon names no glyph),
/// and it is only rebuilt when the `.text`/`.icon` pair changes.
struct SegmentLabelCache {
    /// Fold of the caption and the icon name the entry was built from.
    key: u64,
    /// The caption to paint.
    text: String,
    /// Its measured advance width.
    width: u32,
    /// Its measured line height.
    height: u32,
}

impl SegmentedButton {
    /// Rebuilds one segment's composed caption when its text or icon name changes (K-P15).
    ///
    /// The caption is the segment's own text unless the icon names no glyph, in which case the
    /// crate's text convention prefixes the name. Either way the string and its measurement are
    /// kept until an input changes, so a static frame does neither the clone nor the `format!`.
    fn refresh_segment_labels(&mut self, font: &crate::core::Font, context: &RenderContext) {
        if self.segment_labels.len() != self.segments.len() {
            self.segment_labels.resize_with(self.segments.len(), || None);
        }
        for (index, segment) in self.segments.iter().enumerate() {
            let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
            let mut mix = |value: u64| {
                hash ^= value;
                hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
            };
            for byte in segment.text.as_bytes() {
                mix(u64::from(*byte));
            }
            mix(0xFF);
            if let Some(icon) = segment.icon.as_deref() {
                let drawable = crate::widget::icon_token_is_drawable(icon);
                mix(u64::from(drawable));
                for byte in icon.as_bytes() {
                    mix(u64::from(*byte));
                }
            }
            for byte in font.family().as_bytes() {
                mix(u64::from(*byte));
            }
            mix(u64::from(font.size().to_bits()));
            mix(u64::from(font.weight()));

            if self.segment_labels[index].as_ref().is_some_and(|entry| entry.key == hash) {
                continue;
            }
            let mut text = String::new();
            if let Some(icon) = segment.icon.as_deref() {
                if !crate::widget::icon_token_is_drawable(icon) && !icon.is_empty() {
                    let _ = core::fmt::Write::write_fmt(
                        &mut text,
                        format_args!("{icon} {}", segment.text),
                    );
                }
            }
            if text.is_empty() {
                text.push_str(&segment.text);
            }
            let metrics = context.measure_text(&text, font);
            self.segment_labels[index] = Some(SegmentLabelCache {
                key: hash,
                text,
                width: metrics.width,
                height: metrics.height,
            });
        }
    }

    /// Registers the hovered segment's hint with the hover pipeline (MISC-15).
    ///
    /// An explicit `tooltip` wins; otherwise nothing is registered — a segment's caption is its own
    /// label, and elided captions are the only case where the full text is otherwise unreadable,
    /// which is why the caller can state one.
    fn sync_hover_tooltip(&mut self) {
        let Some(index) = self.hovered_segment else {
            if self.tooltip_source.take().is_some() {
                self.tooltips.note_leave();
            }
            return;
        };
        let text = self
            .segments
            .get(index)
            .and_then(Segment::tooltip)
            .map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_string);
        match text {
            Some(text) => {
                let id = hover_id(self.base.id(), index);
                self.tooltips.register(id, text);
                self.tooltips.note_hover(id);
                self.tooltip_source = Some(id);
            }
            None => {
                if self.tooltip_source.take().is_some() {
                    self.tooltips.note_leave();
                }
            }
        }
    }

    /// Whether a tooltip is still waiting for its delay (or visible).
    fn tooltip_owes_frames(&self) -> bool {
        self.tooltip_source.is_some() || self.tooltips.tooltip().is_some()
    }

    /// Moves the keyboard focus by `delta`, wrapping inside the segment list.
    fn move_focus(&mut self, delta: i32) {
        let count = self.segments.len();
        if count == 0 {
            return;
        }
        let current = self.focus_index.unwrap_or(0) as i32;
        let next = (current + delta).rem_euclid(count as i32) as usize;
        if self.focus_index != Some(next) {
            self.focus_index = Some(next);
            self.base.request_redraw();
        }
    }

    /// Toggles or selects `index` through the same state writes a click uses (MISC-14).
    fn activate_segment(&mut self, index: usize) {
        let Some(segment) = self.segments.get(index) else {
            return;
        };
        if !segment.enabled {
            return;
        }
        if self.allows_multiple {
            if let Some(at) = self.selected.iter().position(|sel| *sel == index) {
                self.selected.remove(at);
            } else {
                self.selected.push(index);
                self.selected.sort_unstable();
            }
        } else {
            self.selected.clear();
            self.selected.push(index);
        }
        self.current_changed.emit(index);
        self.base.request_redraw();
    }
    /// Creates a new SegmentedButton widget with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::SegmentedButton, geometry, "SegmentedButton"),
            segments: Vec::new(),
            selected: Vec::new(),
            allows_multiple: false,
            group_id: None,
            group_generation: 0,
            current_changed: Signal1::new(),
            hovered_segment: None,
            focus_index: None,
            segment_labels: Vec::new(),
            tooltips: crate::widget::companions::TooltipManager::new(geometry),
            tooltip_source: None,
            tooltip_now_ms: 0,
        }
    }

    /// Compatibility spelling of [`Self::current_changed`] (W-20): the same emitted signal.
    pub fn selected_changed(&self) -> &Signal1<usize> {
        &self.current_changed
    }

    /// Returns the **primary** selected segment index, or `None` if none is selected.
    ///
    /// In single-select mode this is the only selection. In multi-select mode it is the lowest
    /// selected index — the one a caller that only knows the single-selection API should act on.
    /// Use [`Self::selected_indices`] to read the whole set.
    pub fn selected_index(&self) -> Option<usize> {
        if self.group_owned_elsewhere() {
            return None;
        }
        self.selected.first().copied()
    }

    /// Returns every selected segment index, ascending.
    ///
    /// The read-back for multi-select: `selected_index` cannot express a set, so a caller that
    /// turned on [`Self::set_allows_multiple`] had no way to learn which segments were chosen.
    pub fn selected_indices(&self) -> &[usize] {
        if self.group_owned_elsewhere() {
            return &[];
        }
        &self.selected
    }

    /// Returns whether `index` is selected.
    pub fn is_selected(&self, index: usize) -> bool {
        !self.group_owned_elsewhere() && self.selected.contains(&index)
    }

    /// The mutually exclusive group this control belongs to.
    pub fn group_id(&self) -> Option<&str> {
        self.group_id.as_deref()
    }

    /// Joins a mutually exclusive group, or leaves it with `None`.
    ///
    /// Joining does not clear the local selection; the next selection in any member wins and
    /// causes the other members to report no selection (see `Self::group_owned_elsewhere`).
    pub fn set_group_id(&mut self, group: Option<String>) {
        if self.group_id == group {
            return;
        }
        self.group_id = group.filter(|name| !name.is_empty());
        self.group_generation = 0;
        self.base.request_redraw();
    }

    /// Whether another control in this group made the authoritative selection.
    fn group_owned_elsewhere(&self) -> bool {
        let Some(group) = self.group_id.as_deref() else {
            return false;
        };
        let Ok(groups) = segmented_groups().lock() else {
            return false;
        };
        match groups.get(group) {
            Some((generation, owner, _)) => {
                *generation > self.group_generation && *owner != self.base.id()
            }
            None => false,
        }
    }

    /// Publishes this control's selection as the group's current value.
    fn publish_group_selection(&mut self) {
        let Some(group) = self.group_id.clone() else {
            return;
        };
        let Ok(mut groups) = segmented_groups().lock() else {
            return;
        };
        let entry = groups.entry(group).or_insert((0, self.base.id(), None));
        entry.0 = entry.0.wrapping_add(1);
        entry.1 = self.base.id();
        entry.2 = self.selected.first().copied();
        self.group_generation = entry.0;
        // A selection in one member changes the painted state of every other member.
        crate::widget::runtime::arm_global_repaint();
    }

    /// Sets the selected segment index, replacing any previous selection.
    ///
    /// Pass `None` to deselect everything. An index past the last segment is ignored (the
    /// selection is cleared), matching the previous clamping behaviour.
    pub fn set_selected_index(&mut self, index: Option<usize>) {
        let next: Vec<usize> = index.filter(|i| *i < self.segments.len()).into_iter().collect();
        if next != self.selected {
            self.selected = next;
            if let Some(idx) = self.selected.first().copied() {
                self.current_changed.emit(idx);
            }
            self.publish_group_selection();
            self.base.request_redraw();
        }
    }

    /// Sets the whole selection at once, **ignoring `allows_multiple`**.
    ///
    /// A programmatic setter is not a gesture: a caller that wants two segments selected has said
    /// so, and silently dropping the extra indices (as a mode-respecting setter would) is the same
    /// "promised but not representable" defect in a different place. Indices past the last segment
    /// are dropped, the set is sorted and deduplicated, and the signal fires once with the lowest
    /// selected index.
    pub fn set_selected_indices(&mut self, indices: impl IntoIterator<Item = usize>) {
        let mut next: Vec<usize> =
            indices.into_iter().filter(|i| *i < self.segments.len()).collect();
        next.sort_unstable();
        next.dedup();
        if next != self.selected {
            self.selected = next;
            if let Some(idx) = self.selected.first().copied() {
                self.current_changed.emit(idx);
            }
            self.publish_group_selection();
            self.base.request_redraw();
        }
    }

    /// Clears the selection.
    pub fn clear_selection(&mut self) {
        self.set_selected_index(None);
    }

    /// Returns whether multiple segments can be selected at once.
    pub fn allows_multiple(&self) -> bool {
        self.allows_multiple
    }

    /// Sets whether multiple segments can be selected.
    ///
    /// Turning multi-select **off** keeps the lowest selection and drops the rest, because a
    /// single-select control holding several selections would paint several highlights that its
    /// own click handler could not produce.
    pub fn set_allows_multiple(&mut self, allows: bool) {
        if self.allows_multiple == allows {
            return;
        }
        self.allows_multiple = allows;
        if !allows && self.selected.len() > 1 {
            self.selected.truncate(1);
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
            // Re-base the selection: the removed index is dropped, everything above it shifts down
            // by one. Doing this on the whole set is what keeps multi-select consistent (the old
            // single-slot adjustment could only follow one row).
            self.selected.retain(|sel| *sel != index);
            for sel in &mut self.selected {
                if *sel > index {
                    *sel -= 1;
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
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn size_hint(&self) -> crate::core::Size {
        crate::core::Size::new(200, 32)
    }

    /// Segments are individually addressable and the control has a keyboard path, so it belongs in
    /// the tab order whatever the kind table says (MISC-14).
    fn is_focusable(&self) -> bool {
        true
    }

    /// Advances the tooltip clocks; `true` while a hint is pending or visible.
    fn tick(&mut self, delta_ms: u32) -> bool {
        self.tooltip_now_ms = self.tooltip_now_ms.saturating_add(u64::from(delta_ms));
        let changed = self.tooltips.tick(u64::from(delta_ms), self.tooltip_now_ms);
        changed || self.tooltip_owes_frames()
    }

    /// Only a pending or visible tooltip owes frames.
    fn is_animating(&self) -> bool {
        self.tooltip_owes_frames()
    }

    impl_draw_bridge!();
    impl_widget_property_hooks!();
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why this is explicit
    ///
    /// `connect_event` validates a name against the capability table and registers a hub slot; only
    /// `event_signal_dyn` joins that name to the signal the control actually **emits**. Without an
    /// arm a published name is valid and inert, which is the silent failure
    /// `tools/check_event_signal_dyn.sh` exists to make impossible: the arm set is compared against
    /// the capability's published names, so the two cannot drift.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "current_changed" => {
                Some(EventSignalRef::mapped("current_changed", &self.current_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `SegmentedButton`'s property contract.
///
/// Read/write semantics are carried over unchanged from the centralised
/// `access_read_dialog.in.rs` / `access_write_dialog.in.rs` dispatch, so callers see
/// the same coercions and the same errors as before. `selected_index` reads back as
/// the real selection (or `Null` when nothing is selected) rather than the legacy
/// hardcoded zero, and `segment_count` is derived from the segment list, so it is
/// readable but read-only.
///
/// `selected_indices` and `allows_multiple` are the multi-select pair: the first is the set as
/// comma-joined indices (the spelling `list_box` uses), the second is the mode flag. Without them a
/// host could turn multi-select on and then read back only *one* of the segments it had chosen.
impl WidgetProperties for SegmentedButton {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "selected_indices" => Ok(CapabilityValue::String(
                self.selected_indices()
                    .iter()
                    .map(|index| index.to_string())
                    .collect::<Vec<_>>()
                    .join(","),
            )),
            "allows_multiple" => Ok(CapabilityValue::Bool(self.allows_multiple())),
            "segment_count" => Ok(CapabilityValue::UInt(self.segment_count() as u64)),
            "group_id" => Ok(match self.group_id() {
                Some(group) => CapabilityValue::String(group.to_string()),
                None => CapabilityValue::Null,
            }),
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
            "selected_indices" => {
                let text = expect_string(value)?;
                let indices = text
                    .split(',')
                    .map(str::trim)
                    .filter(|part| !part.is_empty())
                    .map(|part| {
                        part.parse::<usize>().map_err(|_| CapabilityAccessError::TypeMismatch)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                self.set_selected_indices(indices);
                Ok(())
            }
            "allows_multiple" => {
                self.set_allows_multiple(expect_bool(value)?);
                Ok(())
            }
            "segment_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            "group_id" => {
                match value {
                    CapabilityValue::Null => self.set_group_id(None),
                    other => self.set_group_id(Some(expect_string(other)?)),
                }
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "selected_index",
            "selected_indices",
            "allows_multiple",
            "segment_count",
            "group_id",
            BASE_PROPERTY_NAMES
        ]
    }
}

/// The stable hover id for one segment.
///
/// The tooltip pipeline addresses a hover source by id; a segment is not a widget, so the id is
/// derived from the control's own and the segment's index. The mix is the toolbar's.
fn hover_id(owner: crate::core::ObjectId, index: usize) -> crate::core::ObjectId {
    owner.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (index as u64 + 1)
}

/// Elides a segment caption to `max_width` using the renderer's own measurement.
///
/// The primitive owns the character-boundary rule and the shared `…`; a caption that fits
/// is returned borrowed, so drawing the common case allocates nothing.
fn fitted_caption<'a>(
    caption: &'a str,
    max_width: u32,
    measure: &mut dyn FnMut(&str) -> u32,
) -> Cow<'a, str> {
    elide_to_width(caption, max_width, measure)
}

impl Draw for SegmentedButton {
    fn draw(&mut self, context: &mut RenderContext) {
        let rect = self.geometry();
        if rect.width == 0 || rect.height == 0 {
            return;
        }
        // Composed captions are refreshed before the frame borrows `self` through the style, so
        // an unchanged segment does no cloning, formatting or measuring here (K-P15). The font
        // the draw uses is the one the cache measures with.
        let label_font = role_font(self.base.style(), 13.0);
        self.refresh_segment_labels(&label_font, context);

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then a literal. Every colour below used to be a literal, so a
        // light/dark switch left the control unchanged — the rendering census reported it as
        // theme-blind.
        //
        // Both lookups are the cached, revision-validated forms, so this frame reads the palette
        // once and neither call takes the manager lock (K-P17).
        // `segmented_button` is absent from `WidgetRole::for_kind_name`'s table, so it classifies
        // as `Surface` and the active theme writes the window fill into `style.background_color`.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("segmented_button");
        let (window_fill, foreground, secondary, primary) = match crate::style::palette_shared() {
            Some(colors) => {
                (colors.background, colors.foreground, colors.secondary, colors.primary)
            }
            None => (
                Color::rgb(240, 240, 240),
                Color::BLACK,
                Color::rgb(158, 158, 158),
                Color::rgb(33, 150, 243),
            ),
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
        let font = role_font(style, 13.0);

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

            let is_selected = self.is_selected(i);
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

            // ── Icon ──
            //
            // `Segment::icon` was stored, documented as "displayed as a simple geometric shape",
            // and never drawn -- dead state. It now resolves through the same `icon_token_is_drawable`
            // the navigation bars and tab strips use: a name that names an icon is drawn as geometry
            // in a leading slot, and a string that names none keeps the crate's text convention so a
            // symbol a caller still passes is shown rather than silently dropped.
            let icon_side = SEGMENT_ICON_SIDE.min(track_rect.height.saturating_sub(8)).max(1);
            // A drawable icon owns a leading slot; any other non-empty name was composed into the
            // caption by the cache, which is where the formatting happens now (K-P15).
            let drawable_icon =
                segment.icon.as_deref().filter(|name| crate::widget::icon_token_is_drawable(name));
            let icon_width = if drawable_icon.is_some() { icon_side } else { 0 };
            let icon_gap = if icon_width > 0 { SEGMENT_ICON_GAP } else { 0 };
            let cached = self.segment_labels.get(i).and_then(Option::as_ref);
            let label = cached.map(|entry| entry.text.as_str()).unwrap_or(segment.text.as_str());
            let label_width = cached.map(|entry| entry.width).unwrap_or(0);
            let label_height = cached.map(|entry| entry.height).unwrap_or(0);

            // The icon, its gap and the caption are centred as one group, so adding an icon does not
            // shift the caption off the segment's middle.
            let group_width = icon_width as i32 + icon_gap as i32 + label_width as i32;
            let start_x = seg_rect.x + (seg_width as i32 - group_width) / 2;
            // The origin is the glyph box's top edge, so centring on the segment is half the
            // *line box*; the old `+ ascent` began the box half a line below the middle.
            let text_y = seg_rect.y + (track_rect.height as i32 - label_height as i32) / 2;

            if icon_width > 0 {
                if let Some(name) = drawable_icon {
                    let icon_rect = Rect::new(
                        start_x.max(seg_rect.x),
                        seg_rect.y + (track_rect.height as i32 - icon_width as i32) / 2,
                        icon_width,
                        icon_width,
                    );
                    if !crate::widget::draw_icon_named(context, icon_rect, text_color, name) {
                        log::warn!("segment icon '{name}' names no icon and was not drawn");
                    }
                }
            }

            if !label.is_empty() {
                let label_x = (start_x + icon_width as i32 + icon_gap as i32).max(seg_rect.x);
                let label_y = text_y.max(seg_rect.y);
                let label_right = seg_rect.x + seg_width as i32 - 2;
                // A caption wider than its segment is elided rather than painted over the
                // neighbouring segment. The shared primitive owns the budget and is measured
                // with the renderer's callback, so the fitting decision and the painted advance
                // come from one metric; a caption that fits is borrowed and drawn at its old
                // origin.
                let caption = fitted_caption(
                    label,
                    (label_right - label_x).max(0) as u32,
                    &mut |text: &str| context.measure_text(text, &font).width,
                );
                context.draw_text(
                    Point::new(label_x, label_y),
                    &caption,
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
        // The base keeps the control-level facts (`hovered`, `pressed`, `focus_reason`) and its
        // `MouseEnter`/`MouseLeave` arms are what make `widget_state()` answer `Hover` here. This
        // handler forwarded only in its catch-all arm, so every event it consumed left the base
        // untold — the theme's `"segmented_button:hover"` entry could never fire. Same shape as
        // `bezier_curve_editor` / `refresh_control` / the three view controls.
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        match event {
            // The button and the segment are tested in the arm's **pattern**, not as nested `if`s,
            // so "a left press on an enabled segment" reads as one condition and clippy's
            // collapsible-match lint has nothing left to collapse.
            Event::MousePress { pos, button: 1, .. }
                if self.hit_segment(*pos).is_some_and(|index| {
                    self.segments.get(index).is_some_and(|segment| segment.enabled)
                }) =>
            {
                // The guard above proved the hit is a live segment, so the unwrap is total; it is
                // written as a `let Some` rather than `expect` so a future change to the guard
                // cannot turn into a panic in an event handler.
                if let Some(index) = self.hit_segment(*pos) {
                    // A press also puts the keyboard on that segment, so the two input paths
                    // continue from the same place (MISC-14).
                    self.focus_index = Some(index);
                    self.activate_segment(index);
                }
            }
            // The hover hint follows the segment under the pointer (MISC-15).
            Event::MouseMove { pos } => {
                let hovered = self.hit_segment(*pos);
                if hovered != self.hovered_segment {
                    self.hovered_segment = hovered;
                    self.sync_hover_tooltip();
                    self.base.request_redraw();
                }
                self.tooltips.note_pointer(*pos);
            }
            Event::MouseLeave { .. } => {
                self.hovered_segment = None;
                self.tooltip_source = None;
                self.tooltips.note_leave();
            }
            // The keyboard path (MISC-14): in single-select mode the arrows move the selection
            // itself, so one key press does what one click does; in multi-select mode they move a
            // focus and Space toggles the focused segment, because moving the selection with an
            // arrow would silently change the value.
            Event::KeyPress { key, .. } => match *key {
                37 => {
                    if self.allows_multiple {
                        self.move_focus(-1);
                    } else {
                        let count = self.segments.len();
                        if count > 0 {
                            // With nothing selected yet, the first arrow chooses an end rather than
                            // wrapping from an index no one chose.
                            let next = match self.selected.first() {
                                Some(current) => (*current + count - 1) % count,
                                None => count - 1,
                            };
                            self.focus_index = Some(next);
                            self.activate_segment(next);
                        }
                    }
                }
                39 => {
                    if self.allows_multiple {
                        self.move_focus(1);
                    } else {
                        let count = self.segments.len();
                        if count > 0 {
                            let next = match self.selected.first() {
                                Some(current) => (current + 1) % count,
                                None => 0,
                            };
                            self.focus_index = Some(next);
                            self.activate_segment(next);
                        }
                    }
                }
                32 | 13 if self.allows_multiple => {
                    if let Some(index) = self.focus_index {
                        self.activate_segment(index);
                    }
                }
                _ => {}
            },
            _ => { /* Other events need no control-specific handling */ }
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

    /// Multi-select must be able to hold **more than one** selection.
    ///
    /// `allows_multiple` was a flag over a single `Option<usize>`, so turning it on promised a
    /// behaviour the state could not represent: clicking a second segment replaced the first. The
    /// selection is now a set, and this asserts the set really grows — which is the property the
    /// flag's own name claims and the old shape could never satisfy.
    #[test]
    fn multi_select_holds_more_than_one_segment() {
        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        for label in ["A", "B", "C"] {
            btn.add_segment(Segment::new(label));
        }
        btn.set_allows_multiple(true);

        // A click toggles; a second click adds rather than replaces.
        let segment_width = 300 / 3;
        btn.handle_event(&Event::MousePress { pos: Point::new(10, 18), button: 1, modifiers: 0 });
        assert_eq!(btn.selected_indices(), &[0]);
        btn.handle_event(&Event::MousePress {
            pos: Point::new(segment_width + 10, 18),
            button: 1,
            modifiers: 0,
        });
        assert_eq!(
            btn.selected_indices(),
            &[0, 1],
            "multi-select must be able to hold two segments, not one"
        );
        assert_eq!(btn.selected_index(), Some(0), "the primary selection is the lowest");

        // Clicking a selected segment toggles it back off, leaving the others.
        btn.handle_event(&Event::MousePress { pos: Point::new(10, 18), button: 1, modifiers: 0 });
        assert_eq!(btn.selected_indices(), &[1]);

        // Turning multi-select off keeps only the primary selection, so the control never paints
        // two highlights its own click handler could not produce.
        btn.set_allows_multiple(false);
        assert_eq!(btn.selected_indices(), &[1]);
        assert!(!btn.allows_multiple());
    }

    /// The multi-select set round-trips through the property contract.
    #[test]
    fn the_selection_set_round_trips_through_the_property_api() {
        use crate::widget::capability::WidgetProperties;
        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        for label in ["A", "B", "C"] {
            btn.add_segment(Segment::new(label));
        }
        WidgetProperties::set(&mut btn, "allows_multiple", CapabilityValue::Bool(true)).unwrap();
        WidgetProperties::set(&mut btn, "selected_indices", CapabilityValue::String("2,0".into()))
            .unwrap();
        let read = WidgetProperties::get(&btn, "selected_indices").unwrap();
        assert_eq!(read, CapabilityValue::String("0,2".into()), "stored sorted and deduplicated");
        assert_eq!(btn.selected_indices(), &[0, 2]);
        // An out-of-range index is ignored rather than panicking.
        WidgetProperties::set(&mut btn, "selected_indices", CapabilityValue::String("0,9".into()))
            .unwrap();
        assert_eq!(btn.selected_indices(), &[0]);
    }

    #[test]
    fn segmented_button_current_changed_signal() {
        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        btn.add_segment(Segment::new("A"));
        btn.add_segment(Segment::new("B"));

        let captured = Arc::new(Mutex::new(None));
        btn.current_changed.connect({
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

    // ── Segment icons ──────────────────────────────────────────────────────

    /// A segment icon used to be stored and never drawn -- dead state the doc still described as
    /// "displayed as a simple geometric shape".
    #[test]
    fn a_segment_icon_is_actually_drawn() {
        let mut plain = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        plain.add_segment(Segment::new("Left"));
        let without = crate::widget::svg::render_to_svg(&mut plain);

        let mut with_icon = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        with_icon.add_segment(Segment::new("Left").with_icon("info"));
        let with = crate::widget::svg::render_to_svg(&mut with_icon);

        assert_eq!(
            with.matches("<path").count(),
            without.matches("<path").count() + 1,
            "a segment icon must add exactly one drawn glyph"
        );
    }

    /// A string that names no icon keeps the crate's text convention instead of vanishing.
    #[test]
    fn an_unknown_segment_icon_string_is_shown_as_text() {
        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        btn.add_segment(Segment::new("Left").with_icon("\u{2605}"));
        // The star resolves to no icon here, so it must still be visible in the caption rather than
        // silently dropped -- silently dropping it is what the old code did to every icon.
        let svg = crate::widget::svg::render_to_svg(&mut btn);
        assert!(svg.contains("<path") || svg.contains("<text"), "the segment must still paint");
        let mut plain = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        plain.add_segment(Segment::new("Left"));
        let without = crate::widget::svg::render_to_svg(&mut plain);
        assert_ne!(svg, without, "an unknown icon string must change the drawn caption");
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

    /// The pointer reaching the control has to reach the *base* too.
    ///
    /// Regression: this handler forwarded only in its catch-all arm, so the events it consumed
    /// never recorded the pointer fact and `widget_state()` answered `Normal` for the whole
    /// interaction — the theme's `"segmented_button:hover"` entry was a key nothing could spell.
    #[test]
    fn pointer_state_reaches_the_base() {
        use crate::style::WidgetState;

        let mut btn = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        assert_eq!(btn.widget_state(), WidgetState::Normal);
        btn.handle_event(&Event::MouseEnter { pos: Point::new(10, 10) });
        assert_eq!(
            btn.widget_state(),
            WidgetState::Hover,
            "the base must have been told the pointer arrived"
        );
        btn.handle_event(&Event::MouseLeave { pos: Point::new(500, 500) });
        assert_eq!(btn.widget_state(), WidgetState::Normal, "and that it left");
    }

    /// A multi-byte caption is elided on character boundaries, never inside a glyph.
    ///
    /// The budget is the distance from the caption's origin to the segment's inner right
    /// edge; the shared primitive keeps whole characters and marks the cut with `…`.
    #[test]
    fn a_wide_segment_caption_is_elided_on_character_boundaries() {
        let mut measure = |text: &str| text.chars().count() as u32 * 7;
        // A 35 px budget keeps four characters plus the ellipsis.
        assert_eq!(
            fitted_caption("\u{4e16}\u{754c}\u{4f60}\u{597d}\u{4e16}\u{754c}", 35, &mut measure),
            "\u{4e16}\u{754c}\u{4f60}\u{597d}\u{2026}"
        );
        assert_eq!(
            fitted_caption(
                "\u{1f389}\u{1f389}\u{1f389}\u{1f389}\u{1f389}\u{1f389}",
                35,
                &mut measure
            ),
            "\u{1f389}\u{1f389}\u{1f389}\u{1f389}\u{2026}",
            "an emoji is a single character to the budget, not four bytes"
        );
    }

    /// A caption that fits its segment is borrowed, so the paint path allocates nothing.
    #[test]
    fn a_fitting_segment_caption_is_borrowed() {
        let mut measure = |text: &str| text.chars().count() as u32 * 7;
        assert!(matches!(
            fitted_caption("Day", 35, &mut measure),
            crate::compat::Cow::Borrowed("Day")
        ));
    }

    #[test]
    fn a_group_id_makes_two_controls_mutually_exclusive() {
        let mut first = SegmentedButton::new(Rect::new(0, 0, 200, 32));
        let mut second = SegmentedButton::new(Rect::new(0, 0, 200, 32));
        first.add_segment(Segment::new("Day"));
        first.add_segment(Segment::new("Week"));
        second.add_segment(Segment::new("Day"));
        second.add_segment(Segment::new("Week"));
        first.set_group_id(Some("range".to_string()));
        second.set_group_id(Some("range".to_string()));

        first.set_selected_index(Some(0));
        assert!(first.is_selected(0));
        second.set_selected_index(Some(1));
        assert!(second.is_selected(1));
        assert!(!first.is_selected(0), "the second control now owns the group");
        assert_eq!(first.selected_index(), None);
        assert_eq!(second.selected_index(), Some(1));
    }

    /// Single-select: an arrow moves the selection itself (MISC-14).
    #[test]
    fn the_keyboard_moves_a_single_selection() {
        let mut button = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        button.add_segment(Segment::new("One"));
        button.add_segment(Segment::new("Two"));
        button.add_segment(Segment::new("Three"));
        assert_eq!(button.selected_index(), None);

        button.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        assert_eq!(button.selected_index(), Some(0), "the first arrow picks the first segment");
        button.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        assert_eq!(button.selected_index(), Some(1));
        button.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        assert_eq!(button.selected_index(), Some(2));
        button.handle_event(&Event::key_press(crate::event::key_codes::LEFT, 0));
        assert_eq!(button.selected_index(), Some(1));
    }

    /// Multi-select: the arrows move a focus and Space toggles the focused segment.
    #[test]
    fn the_keyboard_toggles_in_multi_select_mode() {
        let mut button = SegmentedButton::new(Rect::new(0, 0, 300, 36));
        button.set_allows_multiple(true);
        button.add_segment(Segment::new("Bold"));
        button.add_segment(Segment::new("Italic"));

        button.handle_event(&Event::key_press(crate::event::key_codes::RIGHT, 0));
        button.handle_event(&Event::key_press(crate::event::key_codes::SPACE, 0));
        assert_eq!(button.selected_index(), Some(1));
        button.handle_event(&Event::key_press(crate::event::key_codes::SPACE, 0));
        assert_eq!(button.selected_index(), None, "a second Space clears it");
    }

    /// A segment carries a hover hint (MISC-15).
    #[test]
    fn a_segment_can_carry_a_tooltip() {
        let segment = Segment::new("Long caption").with_tooltip("The full sentence");
        assert_eq!(segment.tooltip(), Some("The full sentence"));
        assert_eq!(Segment::new("Plain").tooltip(), None);
    }
}
