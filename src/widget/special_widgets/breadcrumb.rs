// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Breadcrumb navigation widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_string;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, Widget, WidgetKind};
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

/// The declared shape of one breadcrumb segment, as a record.
///
/// This is the row shape of the `segments` property, declared beside the struct it
/// describes (`Chip`'s `CHIP_ITEM_SCHEMA` follows the same rule) so the field names cannot
/// drift from [`BreadcrumbSegment`]'s own fields. Both are required because
/// [`BreadcrumbSegment::new`] requires them, and `id` is the identity a selection is held by.
pub const BREADCRUMB_SEGMENT_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "id",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::required(
                "label",
                crate::widget::capability::PropertyValueKind::String,
            ),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

fn breadcrumb_segment_record(
    segment: &BreadcrumbSegment,
) -> crate::widget::capability::RecordValue {
    crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::String(segment.id.clone()))
        .with("label", CapabilityValue::String(segment.label.clone()))
}

fn breadcrumb_segment_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<BreadcrumbSegment, CapabilityAccessError> {
    let text = |name: &str| match record.get(name) {
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        _ => Err(CapabilityAccessError::TypeMismatch),
    };
    Ok(BreadcrumbSegment { id: text("id")?, label: text("label")? })
}

/// Breadcrumb navigation control with keyboard and mouse interaction.
pub struct Breadcrumb {
    base: BaseWidget,
    segments: Vec<BreadcrumbSegment>,
    selected_index: Option<usize>,
    segment_padding: i32,
    separator_width: i32,
    /// The path separator used when the trail is edited as text.
    separator: String,
    /// Whether the caller asked for the full path to be shown despite the band's width.
    overflow_expanded: bool,
    /// Whether the hidden-segment popup is open (SP-118).
    overflow_open: bool,
    /// The one layout the paint, the hit test and the overflow marker read.
    layout_cache: Option<BreadcrumbLayout>,
    /// Whether the host wants the path in editable form.
    editing: bool,
    /// The text being edited while [`Self::editing`] is set.
    edit_buffer: String,
    /// The editor's insertion point, as a character index into `edit_buffer` (SP-70).
    caret_pos: usize,
    /// Emitted when a segment is activated. Payload is segment id.
    pub segment_activated: Signal1<String>,
    /// Emitted with the segment id when a context menu is requested over one.
    pub context_menu_requested: Signal1<String>,
    /// Emitted with the typed path when an edit is committed.
    pub path_edited: Signal1<String>,
    /// Emitted with the number of segments hidden behind the `…` marker when it is pressed.
    pub overflow_clicked: Signal1<usize>,
}

/// The cached breadcrumb layout.
///
/// The slots used to be recomputed by every `segment_rect_at` call, so a hit test over `n`
/// segments rebuilt and allocated the slot list `n` times. The slot list is a pure function
/// of the band, the resolved font, the padding and the separator width, so it is built once
/// and reused until one of those (or the segment list) changes.
struct BreadcrumbLayout {
    band: Rect,
    font: Font,
    padding: i32,
    separator_width: i32,
    overflow_expanded: bool,
    slots: Vec<(Option<usize>, i32)>,
}

/// The byte offset of character `index` in `text`, or the text's length when it is past the end.
fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices().nth(index).map(|(byte, _)| byte).unwrap_or(text.len())
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
            separator: String::from("/"),
            overflow_expanded: false,
            overflow_open: false,
            layout_cache: None,
            editing: false,
            edit_buffer: String::new(),
            caret_pos: 0,
            segment_activated: Signal1::new(),
            context_menu_requested: Signal1::new(),
            path_edited: Signal1::new(),
            overflow_clicked: Signal1::new(),
        }
    }

    /// Starts or stops path editing.
    ///
    /// While editing, the trail is replaced by the editable text so the user can retype the
    /// whole path. Committing emits `path_edited` with the typed string; the host owns the
    /// parsing because only it knows the path separator convention for its own paths.
    pub fn set_editing(&mut self, editing: bool) {
        if self.editing == editing {
            return;
        }
        self.editing = editing;
        if editing {
            self.edit_buffer = self
                .segments
                .iter()
                .map(|segment| segment.label.as_str())
                .collect::<Vec<_>>()
                .join(&self.separator);
            // Entering edit mode puts the caret after the existing path, which is where a user
            // who opened an address bar expects to start typing (SP-70).
            self.caret_pos = self.edit_buffer.chars().count();
        }
        self.base.request_redraw();
    }

    /// Returns whether the path is in editable form.
    pub fn is_editing(&self) -> bool {
        self.editing
    }

    /// Returns the text currently held by the path editor.
    pub fn editing_text(&self) -> &str {
        &self.edit_buffer
    }

    /// The editor's insertion point, as a character index into [`Self::editing_text`] (SP-70).
    pub fn caret_pos(&self) -> usize {
        self.caret_pos
    }

    /// Moves the editor's caret, clamped to the text.
    pub fn set_caret_pos(&mut self, position: usize) {
        let length = self.edit_buffer.chars().count();
        let position = position.min(length);
        if self.caret_pos == position {
            return;
        }
        self.caret_pos = position;
        self.base.request_redraw();
    }

    /// Inserts `text` at the caret and leaves the caret after it.
    pub fn insert_at_caret(&mut self, text: &str) {
        let byte = char_to_byte(&self.edit_buffer, self.caret_pos);
        self.edit_buffer.insert_str(byte, text);
        self.caret_pos += text.chars().count();
        self.base.request_redraw();
    }

    /// Deletes the character before the caret, if any.
    pub fn backspace_at_caret(&mut self) {
        if self.caret_pos == 0 {
            return;
        }
        let start = char_to_byte(&self.edit_buffer, self.caret_pos - 1);
        let end = char_to_byte(&self.edit_buffer, self.caret_pos);
        self.edit_buffer.replace_range(start..end, "");
        self.caret_pos -= 1;
        self.base.request_redraw();
    }

    /// Deletes the character after the caret, if any.
    pub fn delete_at_caret(&mut self) {
        let length = self.edit_buffer.chars().count();
        if self.caret_pos >= length {
            return;
        }
        let start = char_to_byte(&self.edit_buffer, self.caret_pos);
        let end = char_to_byte(&self.edit_buffer, self.caret_pos + 1);
        self.edit_buffer.replace_range(start..end, "");
        self.base.request_redraw();
    }

    /// Commits the edited path, publishing it and leaving edit mode.
    pub fn commit_path_edit(&mut self) {
        if !self.editing {
            return;
        }
        let text = self.edit_buffer.clone();
        self.editing = false;
        self.path_edited.emit(text);
        self.base.request_redraw();
    }

    /// Moves the editor's caret by a signed character count, clamped.
    pub fn move_caret_by(&mut self, delta: isize) {
        let next = (self.caret_pos as isize + delta)
            .clamp(0, self.edit_buffer.chars().count() as isize) as usize;
        self.set_caret_pos(next);
    }

    /// Ends an edit without publishing it, restoring the pre-edit caret.
    pub fn cancel_path_edit(&mut self) {
        if !self.editing {
            return;
        }
        self.editing = false;
        self.edit_buffer.clear();
        self.caret_pos = 0;
        self.base.request_redraw();
    }

    /// The display slots the trail renders: a segment index, or `None` for the collapsed
    /// middle marker.
    ///
    /// When the whole path fits it is laid out one slot per segment. When it does not, the
    /// first and last segments are kept and everything between them becomes one `…` marker —
    /// the convention a file browser uses, and the only way to keep both the root and the
    /// current location visible. The previous layout dropped every segment past the right
    /// edge instead, so the *current* segment — the one that matters — disappeared first.
    ///
    /// The paint and the hit test read [`Self::cached_slots`] directly; this owned form exists for
    /// the layout assertions in this file's tests.
    #[cfg(test)]
    fn display_slots(&mut self) -> Vec<(Option<usize>, i32)> {
        self.ensure_layout();
        self.cached_slots().to_vec()
    }

    /// Rebuilds the layout when the band, font, padding, separator width or expansion changed.
    fn ensure_layout(&mut self) {
        let band = self.band();
        let font = effective_font(self.style()).clone();
        let fresh = self.layout_cache.as_ref().is_some_and(|cache| {
            cache.band == band
                && cache.font == font
                && cache.padding == self.segment_padding
                && cache.separator_width == self.separator_width
                && cache.overflow_expanded == self.overflow_expanded
        });
        if fresh {
            return;
        }
        let slots = self.build_slots(band, &font);
        self.layout_cache = Some(BreadcrumbLayout {
            band,
            font,
            padding: self.segment_padding,
            separator_width: self.separator_width,
            overflow_expanded: self.overflow_expanded,
            slots,
        });
    }

    fn cached_slots(&self) -> &[(Option<usize>, i32)] {
        &self.layout_cache.as_ref().expect("ensure_layout() populates the layout cache").slots
    }

    fn build_slots(&self, band: Rect, font: &Font) -> Vec<(Option<usize>, i32)> {
        let band_width = band.width as i32;
        let all: Vec<(Option<usize>, i32)> = self
            .segments
            .iter()
            .enumerate()
            .map(|(index, segment)| {
                (Some(index), self.segment_width(segment, self.segment_padding, font).max(1))
            })
            .collect();
        let total: i32 = all.iter().map(|(_, width)| *width).sum::<i32>()
            + self.separator_width * all.len().saturating_sub(1) as i32;
        // `overflow_expanded` is the caller's explicit "show the whole path"; the auto rule
        // still applies when nothing fits and the caller has not asked.
        if total <= band_width || self.segments.len() <= 2 || self.overflow_expanded {
            return all;
        }
        let marker =
            (estimate_text_width("…", font, 1.0) as i32 + self.segment_padding * 2).max(16);
        let first = all.first().copied().unwrap_or((None, 0));
        let last = all.last().copied().unwrap_or((None, 0));
        vec![first, (None, marker), last]
    }

    /// The number of segments currently folded behind the `…` marker.
    pub fn collapsed_count(&self) -> usize {
        let visible = match self.layout_cache.as_ref() {
            Some(cache) => cache.slots.iter().filter(|(slot, _)| slot.is_some()).count(),
            None => {
                let band = self.band();
                let font = effective_font(self.style());
                let slots = self.build_slots(band, font);
                slots.iter().filter(|(slot, _)| slot.is_some()).count()
            }
        };
        self.segments.len().saturating_sub(visible)
    }

    /// Shows the full path even when it is wider than the band.
    ///
    /// The extra segments are clipped by the band, but they are laid out and hit-testable,
    /// which is what a host showing its own overflow popup needs them to be. A caller that
    /// wants the automatic fold back calls [`Self::collapse`].
    pub fn expand(&mut self) {
        if self.overflow_expanded {
            return;
        }
        self.overflow_expanded = true;
        self.layout_cache = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Restores the automatic first / `…` / last folding.
    pub fn collapse(&mut self) {
        if !self.overflow_expanded {
            return;
        }
        self.overflow_expanded = false;
        self.layout_cache = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// The width of the hidden-segment popup's row.
    const OVERFLOW_ROW_HEIGHT: u32 = 18;

    /// The padding around the hidden-segment popup's rows.
    const OVERFLOW_PADDING: u32 = 4;

    /// Whether the hidden-segment popup is open.
    pub fn is_overflow_open(&self) -> bool {
        self.overflow_open
    }

    /// Opens the hidden-segment popup instead of showing the whole path.
    ///
    /// The marker stands for the folded middle of the trail; showing only those segments keeps the
    /// root and the current location in place, where expanding the whole path used to push the
    /// current segment out of the band (SP-118).
    pub fn open_overflow(&mut self) {
        if self.overflow_open || self.collapsed_count() == 0 {
            return;
        }
        self.overflow_open = true;
        self.base.request_redraw();
    }

    /// Closes the hidden-segment popup.
    pub fn close_overflow(&mut self) {
        if !self.overflow_open {
            return;
        }
        self.overflow_open = false;
        self.base.request_redraw();
    }

    /// The segments the current fold hides, or an empty slice when nothing is folded.
    pub fn hidden_segments(&self) -> &[BreadcrumbSegment] {
        if self.segments.len() <= 2 || self.collapsed_count() == 0 {
            return &[];
        }
        &self.segments[1..self.segments.len() - 1]
    }

    /// The popup's rectangle: one row per hidden segment, anchored under the trail.
    fn overflow_popup_rect(&self) -> Option<Rect> {
        let hidden = self.hidden_segments().len();
        if !self.overflow_open || hidden == 0 {
            return None;
        }
        let band = self.band();
        let geometry = self.geometry();
        let top = band.y + band.height as i32;
        let available = (geometry.y + geometry.height as i32 - top).max(0) as u32;
        if available == 0 {
            return None;
        }
        let rows = hidden.min((available / Self::OVERFLOW_ROW_HEIGHT).max(1) as usize);
        let height = (rows as u32) * Self::OVERFLOW_ROW_HEIGHT + Self::OVERFLOW_PADDING * 2;
        let width = band.width.min(240).max(80);
        Some(Rect::new(band.x, top, width, height.min(available)))
    }

    /// The segment index under `pos` while the popup is open.
    fn overflow_row_at(&self, pos: Point) -> Option<usize> {
        let popup = self.overflow_popup_rect()?;
        if pos.x < popup.x || pos.x >= popup.x + popup.width as i32 {
            return None;
        }
        let offset = pos.y - popup.y - Self::OVERFLOW_PADDING as i32;
        if offset < 0 {
            return None;
        }
        let row = (offset as u32) / Self::OVERFLOW_ROW_HEIGHT;
        let hidden = self.hidden_segments();
        if (row as usize) >= hidden.len() {
            return None;
        }
        Some(1 + row as usize)
    }

    /// The rectangle one popup row occupies, for tests and a host that draws its own menu.
    pub fn overflow_row_rect(&self, hidden_index: usize) -> Option<Rect> {
        let popup = self.overflow_popup_rect()?;
        if hidden_index >= self.hidden_segments().len() {
            return None;
        }
        let y = popup.y
            + Self::OVERFLOW_PADDING as i32
            + hidden_index as i32 * Self::OVERFLOW_ROW_HEIGHT as i32;
        Some(Rect::new(
            popup.x + Self::OVERFLOW_PADDING as i32,
            y,
            popup.width.saturating_sub(Self::OVERFLOW_PADDING * 2),
            Self::OVERFLOW_ROW_HEIGHT,
        ))
    }

    /// The path separator used when the trail is edited as text.
    pub fn separator(&self) -> &str {
        &self.separator
    }

    /// Sets the path separator used when the trail is edited as text.
    pub fn set_separator(&mut self, separator: impl Into<String>) {
        let separator = separator.into();
        if self.separator == separator {
            return;
        }
        self.separator = separator;
        self.base.request_redraw();
    }

    /// Replaces full segment path.
    pub fn set_segments(&mut self, segments: Vec<BreadcrumbSegment>) {
        self.segments = segments;
        self.selected_index =
            if self.segments.is_empty() { None } else { Some(self.segments.len() - 1) };
        self.layout_cache = None;
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
        self.layout_cache = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Clears segment path.
    pub fn clear_segments(&mut self) {
        self.segments.clear();
        self.selected_index = None;
        self.layout_cache = None;
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

    /// The width one segment claims: its label plus the padding on both sides.
    ///
    /// # Why the shared ruler (D09-CTRL-02)
    ///
    /// The box used `chars().count() * 8`, a flat per-character estimate. The segment box is also
    /// the click target, while the label inside it is drawn with the real glyph advances — so a wide
    /// label (CJK, `W`) could be wider than its box and spill into the next separator/segment while
    /// its overflow sat outside the clickable rectangle. Measuring with [`estimate_text_width`] — the
    /// same ruler the painter uses — makes the box fit the label it draws, so the click target
    /// covers the visible segment.
    fn segment_width(&self, segment: &BreadcrumbSegment, padding: i32, font: &Font) -> i32 {
        // The **effective font** — the resolved theme/caller font — so the segment box and the label
        // are measured with the font the row paints with (D09-STYLE-01 / D09-STYLE-02).
        estimate_text_width(&segment.label, font, 1.0) as i32 + padding * 2
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
    /// Kept for the layout tests; the paint and the hit test walk the cached slot list directly.
    #[cfg(test)]
    fn segment_rect_at(&mut self, index: usize) -> Option<Rect> {
        if index >= self.segments.len() {
            return None;
        }
        let band = self.band();
        let band_right = band.x + band.width as i32;
        self.ensure_layout();
        let mut x = band.x;
        let slots = self.cached_slots();

        for (position, (slot, width)) in slots.iter().enumerate() {
            if x >= band_right {
                break;
            }
            let visible_width = (*width).min(band_right - x).max(0) as u32;
            if *slot == Some(index) {
                if visible_width == 0 {
                    return None;
                }
                return Some(Rect::new(x, band.y, visible_width, band.height));
            }
            x += width;
            if position + 1 < slots.len() {
                x += self.separator_width;
            }
        }

        None
    }

    fn hit_index(&mut self, pos: Point) -> Option<usize> {
        let band = self.band();
        if pos.y < band.y || pos.y >= band.y + band.height as i32 {
            return None;
        }
        if pos.x < band.x || pos.x >= band.x + band.width as i32 {
            return None;
        }
        self.ensure_layout();
        let band_right = band.x + band.width as i32;
        let mut x = band.x;
        let slots = self.cached_slots();
        for (position, (slot, width)) in slots.iter().enumerate() {
            if x >= band_right {
                break;
            }
            let visible_width = (*width).min(band_right - x).max(0);
            if pos.x >= x && pos.x < x + visible_width {
                return *slot;
            }
            x += width;
            if position + 1 < slots.len() {
                x += self.separator_width;
            }
        }
        None
    }

    /// The folded marker's rectangle and the number of segments behind it, if any.
    fn overflow_at(&mut self, pos: Point) -> Option<usize> {
        let band = self.band();
        if pos.y < band.y || pos.y >= band.y + band.height as i32 {
            return None;
        }
        self.ensure_layout();
        let slots = self.cached_slots();
        if !slots.iter().any(|(slot, _)| slot.is_none()) {
            return None;
        }
        let hidden = self.segments.len().saturating_sub(2);
        let band_right = band.x + band.width as i32;
        let mut x = band.x;
        for (position, (slot, width)) in slots.iter().enumerate() {
            if x >= band_right {
                break;
            }
            let visible_width = (*width).min(band_right - x).max(0);
            if slot.is_none() {
                return (pos.x >= x && pos.x < x + visible_width).then_some(hidden);
            }
            x += width;
            if position + 1 < slots.len() {
                x += self.separator_width;
            }
        }
        None
    }
}

impl Widget for Breadcrumb {
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
        crate::core::Size::new(300, 28)
    }

    /// Reports the segment under the pointer to the host's context menu.
    fn context_menu_event(&mut self, pos: Point) -> bool {
        let Some(index) = self.hit_index(pos) else {
            return false;
        };
        let Some(segment) = self.segments.get(index) else {
            return false;
        };
        self.context_menu_requested.emit(segment.id.clone());
        true
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
            "segment_activated" => {
                Some(EventSignalRef::mapped("segment_activated", &self.segment_activated, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "context_menu_requested" => Some(EventSignalRef::mapped(
                "context_menu_requested",
                &self.context_menu_requested,
                |v| crate::widget::capability::CapabilityValue::String(v.clone()),
            )),
            "path_edited" => Some(EventSignalRef::mapped("path_edited", &self.path_edited, |v| {
                crate::widget::capability::CapabilityValue::String(v.clone())
            })),
            "overflow_clicked" => {
                Some(EventSignalRef::mapped("overflow_clicked", &self.overflow_clicked, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `Breadcrumb`'s property contract.
///
/// Read semantics are carried over unchanged from the centralised
/// `access_read_base.in.rs` dispatch. The two counts are derived from the segment path, so
/// writes to them are refused with [`CapabilityAccessError::ReadOnlyProperty`]; the path
/// itself is replaced through the `segments` property (or, for a single change, the
/// `set_segments` / `push_segment` / `clear_segments` methods).
impl WidgetProperties for Breadcrumb {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "segments" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &BREADCRUMB_SEGMENT_SCHEMA,
                    self.segments().iter().map(breadcrumb_segment_record).collect(),
                )))
            }
            "segment_count" => Ok(CapabilityValue::UInt(self.segments().len() as u64)),
            "collapsed_count" => Ok(CapabilityValue::UInt(self.collapsed_count() as u64)),
            "separator" => Ok(CapabilityValue::String(self.separator().to_string())),
            "selected_index" => match self.selected_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "segments" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                // Validated in full before anything is replaced, so a refused write leaves the
                // live path untouched rather than half-swapped.
                list.validate_against(&BREADCRUMB_SEGMENT_SCHEMA).map_err(|violation| {
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("segments"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let mut segments = Vec::with_capacity(list.len());
                for record in list.items() {
                    segments.push(breadcrumb_segment_from_record(record)?);
                }
                self.set_segments(segments);
                Ok(())
            }
            "segment_count" | "selected_index" | "collapsed_count" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            "separator" => {
                self.set_separator(expect_string(value)?);
                Ok(())
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "segments",
            "segment_count",
            "selected_index",
            "collapsed_count",
            "separator",
            BASE_PROPERTY_NAMES
        ]
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

        if self.editing {
            match event {
                Event::KeyPress { key, .. } => match *key {
                    8 => self.backspace_at_caret(),
                    46 => self.delete_at_caret(),
                    13 => self.commit_path_edit(),
                    27 => self.cancel_path_edit(),
                    37 => self.move_caret_by(-1),
                    39 => self.move_caret_by(1),
                    36 => self.set_caret_pos(0),
                    35 => self.set_caret_pos(self.edit_buffer.chars().count()),
                    _ => {}
                },
                // Text arrives after the key arm so Enter/Escape are not also inserted. The
                // editor inserts at the caret instead of appending, which is what makes editing a
                // path in the middle possible (SP-70).
                Event::TextInput { text } | Event::ImeCommit { text } => {
                    self.insert_at_caret(text);
                }
                _ => {}
            }
            return;
        }

        match event {
            Event::MousePress { pos, button: 1, .. } => {
                // While the hidden-segment popup is open, a press in it selects the row and a press
                // anywhere else closes it first (SP-118).
                if self.overflow_open {
                    if let Some(index) = self.overflow_row_at(*pos) {
                        let _ = self.set_selected_index(index);
                        self.close_overflow();
                        self.base.request_redraw();
                        return;
                    }
                    self.close_overflow();
                }
                if let Some(hidden) = self.overflow_at(*pos) {
                    // The marker no longer expands the trail: it lists only the folded middle.
                    self.overflow_clicked.emit(hidden);
                    self.open_overflow();
                    return;
                }
                if let Some(index) = self.hit_index(*pos) {
                    let _ = self.set_selected_index(index);
                }
            }
            Event::MouseDoubleClick { pos, button: 1 } if !self.overflow_open => {
                if let Some(index) = self.hit_index(*pos) {
                    let _ = self.set_selected_index(index);
                    let _ = self.activate_selected();
                }
            }
            Event::ContextMenu { pos } => {
                // The host owns the menu; this signal tells it which segment was asked about.
                if let Some(index) = self.hit_index(*pos) {
                    if let Some(segment) = self.segments.get(index) {
                        self.context_menu_requested.emit(segment.id.clone());
                    }
                }
            }
            Event::KeyPress { key, modifiers } => match *key {
                27 if self.overflow_open => self.close_overflow(),
                37 => self.move_selection(-1),
                39 => self.move_selection(1),
                13 => {
                    let _ = self.activate_selected();
                }
                113 | 2 => self.set_editing(true),
                27 if *modifiers == 0 => self.set_editing(false),
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
        // The layout is resolved before the style borrow because `ensure_layout` writes the
        // cache; everything after this line only reads, so the style can be borrowed rather than
        // cloned, and one font serves every label (SP-P75).
        self.ensure_layout();
        let style = self.base.style();
        let font = effective_font(style);
        let theme = crate::style::resolved_theme_style_shared("breadcrumb");
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
            .unwrap_or_else(|| crate::core::Color::border_of(background));
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

        if self.editing {
            // The editor replaces the trail while it is open. The text is drawn from a scrolled
            // origin so the caret stays visible, and the caret is placed by measuring the text
            // before it rather than by assuming the caret is at the end (SP-70).
            let line = context.text_line(band, font);
            let inner = Rect::new(
                band.x + self.segment_padding,
                line.y,
                band.width.saturating_sub(self.segment_padding as u32 + 2),
                line.height,
            );
            let caret_chars = self.caret_pos.min(self.edit_buffer.chars().count());
            let prefix: String = self.edit_buffer.chars().take(caret_chars).collect();
            let prefix_width = context.measure_text(&prefix, font).width as i32;
            // Keep the caret at least one glyph inside the field: the text is shifted left by
            // whatever the prefix overflows, which is the minimal scroll a line editor needs.
            let overflow = (prefix_width - (inner.width as i32 - 2)).max(0);
            context.draw_text_fitted_cow(
                Rect::new(
                    inner.x - overflow,
                    inner.y,
                    inner.width.saturating_add(overflow as u32),
                    inner.height,
                ),
                &self.edit_buffer,
                font,
                text_color,
                HorizontalAlignment::Left,
            );
            let caret_x = inner.x + prefix_width - overflow;
            context.draw_line_stroke(
                Point::new(caret_x, inner.y),
                Point::new(caret_x, inner.y + inner.height as i32),
                text_color,
                1,
            );
            return;
        }

        let slots = self.cached_slots();
        let mut slot_x = band.x;
        for (position, (slot, width)) in slots.iter().enumerate() {
            let Some(index) = *slot else {
                // Collapsed middle: one marker standing in for the hidden segments.
                let rect = Rect::new(slot_x, band.y, *width as u32, band.height);
                let line = context.text_line(rect, font);
                context.draw_text_fitted_cow(
                    Rect::new(rect.x, line.y, rect.width, line.height),
                    "…",
                    font,
                    separator_color,
                    HorizontalAlignment::Center,
                );
                slot_x += width;
                if position + 1 < slots.len() {
                    slot_x += self.separator_width;
                }
                continue;
            };
            let Some(segment) = self.segments.get(index) else {
                continue;
            };
            let band_right = band.x + band.width as i32;
            let visible = (*width).min(band_right - slot_x).max(0) as u32;
            if visible == 0 {
                slot_x += width;
                if position + 1 < slots.len() {
                    slot_x += self.separator_width;
                }
                continue;
            }
            let segment_rect = Rect::new(slot_x, band.y, visible, band.height);

            if self.selected_index == Some(index) {
                context.fill_rect(segment_rect, selected_background);
            }

            // A label's origin is the glyph box's top edge, so the segment's raw midpoint put
            // that edge on the middle line and drew the label half a line low. The line box
            // centred in the segment gives the origin instead; the separator below shares it.
            //
            // The label is **fitted** into its own segment box (minus the leading pad), so a label
            // wider than the box the segment was laid out as cannot spill into the next segment's
            // separator: the box is also the click target, and D09-CTRL-02 is exactly the case where
            // ink and target disagreed. `estimate_text_width` sizes the box, so this only bites for a
            // caller font larger than the measured one — where eliding is still the correct answer.
            let line = context.text_line(segment_rect, font);
            let label_box = Rect::new(
                segment_rect.x + self.segment_padding,
                line.y,
                (segment_rect.width as i32 - self.segment_padding).max(0) as u32,
                line.height,
            );
            context.draw_text_fitted_cow(
                label_box,
                &segment.label,
                font,
                text_color,
                HorizontalAlignment::Left,
            );

            // The separator sits in the gap after the segment, and only exists when another
            // slot follows it inside the band. A vector chevron reads as a path separator at
            // any text scale; the `>` glyph it replaces changed weight and baseline with the
            // theme's font.
            let gap_x = segment_rect.x + segment_rect.width as i32;
            let has_next = position + 1 < slots.len();
            if has_next && gap_x + self.separator_width <= band.x + band.width as i32 {
                let side = 10u32.min(band.height.max(1));
                let icon_rect = Rect::new(
                    gap_x + (self.separator_width - side as i32) / 2,
                    band.y + (band.height.saturating_sub(side) / 2) as i32,
                    side,
                    side,
                );
                crate::widget::draw_icon_at(
                    context,
                    icon_rect,
                    separator_color,
                    IconName::ChevronRight,
                );
            }
            slot_x += width;
            if position + 1 < slots.len() {
                slot_x += self.separator_width;
            }
        }

        // ── Hidden-segment popup (SP-118) ──
        // The `…` marker lists only the folded middle, leaving the root and the current location
        // where they were; expanding the whole path used to push the last segment out of the band.
        if let Some(popup) = self.overflow_popup_rect() {
            // The popup is a raised surface, so it is a step away from the band rather than the
            // same fill, and it borrows the trail's own resolved ink and border.
            let surface = background.blend(&text_color, 0.06);
            context.fill_rect(popup, surface);
            context.draw_rect(popup, border);
            let hidden = self.hidden_segments();
            for (row, segment) in hidden.iter().enumerate() {
                let Some(rect) = self.overflow_row_rect(row) else {
                    break;
                };
                if self.selected_index == Some(1 + row) {
                    context.fill_rect(rect, selected_background);
                }
                let line = context.text_line(rect, font);
                context.draw_text_fitted_cow(
                    Rect::new(
                        rect.x + self.segment_padding,
                        line.y,
                        rect.width.saturating_sub(self.segment_padding as u32).max(1),
                        line.height,
                    ),
                    &segment.label,
                    font,
                    text_color,
                    HorizontalAlignment::Left,
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Font;
    use std::sync::{Arc, Mutex};

    fn sample_segments() -> Vec<BreadcrumbSegment> {
        vec![
            BreadcrumbSegment::new("root", "Root"),
            BreadcrumbSegment::new("project", "Project"),
            BreadcrumbSegment::new("src", "src"),
        ]
    }

    #[test]
    fn segments_property_round_trips_the_path() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 320, 28));
        let list = crate::widget::capability::RecordList::from_items(
            &BREADCRUMB_SEGMENT_SCHEMA,
            vec![
                crate::widget::capability::RecordValue::new()
                    .with("id", CapabilityValue::String("root".to_string()))
                    .with("label", CapabilityValue::String("Root".to_string())),
                crate::widget::capability::RecordValue::new()
                    .with("id", CapabilityValue::String("src".to_string()))
                    .with("label", CapabilityValue::String("src".to_string())),
            ],
        );
        breadcrumb.set("segments", CapabilityValue::Records(list)).expect("segments is writable");
        assert_eq!(breadcrumb.segments().len(), 2);
        // `set_segments` keeps the arm on the deepest segment, and the property write goes
        // through it rather than restating the rule.
        assert_eq!(breadcrumb.selected_index(), Some(1));
        match breadcrumb.get("segments").expect("segments is readable") {
            CapabilityValue::Records(read) => {
                assert_eq!(read.len(), 2);
                let first = &read.items()[0];
                assert_eq!(first.get("id"), Some(&CapabilityValue::String("root".to_string())));
                assert_eq!(first.get("label"), Some(&CapabilityValue::String("Root".to_string())));
            }
            other => panic!("segments should read back as Records, got {other:?}"),
        }
    }

    #[test]
    fn segments_property_refuses_duplicate_ids_and_keeps_the_live_path() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 320, 28));
        breadcrumb.set_segments(sample_segments());
        let duplicate = crate::widget::capability::RecordList::from_items(
            &BREADCRUMB_SEGMENT_SCHEMA,
            vec![
                crate::widget::capability::RecordValue::new()
                    .with("id", CapabilityValue::String("same".to_string()))
                    .with("label", CapabilityValue::String("One".to_string())),
                crate::widget::capability::RecordValue::new()
                    .with("id", CapabilityValue::String("same".to_string()))
                    .with("label", CapabilityValue::String("Two".to_string())),
            ],
        );
        assert!(matches!(
            breadcrumb.set("segments", CapabilityValue::Records(duplicate)),
            Err(CapabilityAccessError::ConstraintViolation { .. })
        ));
        // Nothing was applied: a refused write leaves the live path untouched.
        assert_eq!(breadcrumb.segments().len(), 3);
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

    // ── D09-CTRL-02: segments are measured with the shared text ruler ──

    /// A wide-glyph segment must be measured by the shared ruler, so its click target covers the
    /// label it draws and it does not spill into the next segment's separator.
    ///
    /// # The defect this pins
    ///
    /// `segment_width` estimated `chars().count() * 8`. A CJK label advances far wider, so the
    /// segment box (which is also the click target) was narrower than the drawn label: the text bled
    /// toward the next separator while its overflow sat outside the clickable rectangle. The box now
    /// uses `estimate_text_width`, the same ruler the painter uses.
    #[test]
    fn a_wide_glyph_segment_is_measured_by_the_shared_ruler() {
        let label = "\u{4e2d}\u{6587}\u{76ee}\u{5f55}"; // four CJK scalars
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 800, 28));
        breadcrumb.set_segments(vec![
            BreadcrumbSegment::new("wide", label),
            BreadcrumbSegment::new("next", "next"),
        ]);

        let measured = estimate_text_width(label, &Font::default(), 1.0) as i32;
        let old_estimate = label.chars().count() as i32 * 8;
        assert!(
            measured > old_estimate,
            "CJK advances wider than the old per-char estimate (was {old_estimate}, measured {measured})"
        );
        let seg = breadcrumb.segment_rect_at(0).expect("a laid-out segment");
        assert!(
            seg.width as i32 >= measured + breadcrumb.segment_padding * 2,
            "the segment must hold the measured label plus padding, got {}",
            seg.width
        );

        // The click target is that same box, and the next segment starts after the separator — no
        // overlap.
        let inside = Point::new(seg.x + seg.width as i32 - 1, seg.y + seg.height as i32 / 2);
        assert_eq!(breadcrumb.hit_index(inside), Some(0));
        let next = breadcrumb.segment_rect_at(1).expect("the second segment");
        assert!(
            next.x >= seg.x + seg.width as i32 + breadcrumb.separator_width,
            "the next segment must clear the first segment and its separator"
        );
    }

    #[test]
    fn an_overflowing_path_keeps_both_ends_and_folds_the_middle() {
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 140, 28));
        breadcrumb.set_segments(vec![
            BreadcrumbSegment::new("root", "Root"),
            BreadcrumbSegment::new("middle", "A long middle directory"),
            BreadcrumbSegment::new("leaf", "Leaf"),
        ]);
        let slots = breadcrumb.display_slots();
        assert_eq!(slots.len(), 3, "first, marker and last");
        assert_eq!(slots[0].0, Some(0));
        assert_eq!(slots[1].0, None, "the middle collapses to a marker");
        assert_eq!(slots[2].0, Some(2));
    }

    /// The folded marker is a control, not decoration: pressing it reports the hidden count and
    /// opens a list of only the hidden segments instead of expanding the whole path (SP-118).
    #[test]
    fn the_overflow_marker_lists_only_the_hidden_segments() {
        use std::sync::{Arc, Mutex};
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 140, 200));
        breadcrumb.set_segments(vec![
            BreadcrumbSegment::new("root", "Root"),
            BreadcrumbSegment::new("middle", "A long middle directory"),
            BreadcrumbSegment::new("leaf", "Leaf"),
        ]);
        assert_eq!(breadcrumb.collapsed_count(), 1);

        let seen = Arc::new(Mutex::new(Vec::<usize>::new()));
        let sink = Arc::clone(&seen);
        breadcrumb.overflow_clicked.connect(move |hidden| sink.lock().unwrap().push(*hidden));

        // Walk the same slot list the paint uses to find the marker's centre.
        let band = breadcrumb.band();
        let slots = breadcrumb.display_slots();
        let mut x = band.x;
        let mut marker_x = None;
        for (position, (slot, width)) in slots.iter().enumerate() {
            if slot.is_none() {
                marker_x = Some(x + width / 2);
            }
            x += width;
            if position + 1 < slots.len() {
                x += breadcrumb.separator_width;
            }
        }
        breadcrumb.handle_event(&Event::mouse_press(
            marker_x.expect("a marker"),
            band.y + band.height as i32 / 2,
            1,
        ));

        assert_eq!(*seen.lock().unwrap(), vec![1]);
        assert!(breadcrumb.is_overflow_open(), "the marker opens the hidden-segment list");
        assert_eq!(breadcrumb.collapsed_count(), 1, "the trail stays folded");
        assert_eq!(
            breadcrumb
                .hidden_segments()
                .iter()
                .map(|segment| segment.id.as_str())
                .collect::<Vec<_>>(),
            vec!["middle"],
            "only the folded middle is listed"
        );

        // Picking a row selects that segment and closes the list without expanding the trail.
        let row = breadcrumb.overflow_row_rect(0).expect("a popup row");
        breadcrumb.handle_event(&Event::mouse_press(row.x + 2, row.y + 2, 1));
        assert_eq!(breadcrumb.selected_index(), Some(1));
        assert!(!breadcrumb.is_overflow_open());
        assert_eq!(breadcrumb.collapsed_count(), 1);

        breadcrumb.set_separator("::");
        breadcrumb.set_editing(true);
        assert_eq!(breadcrumb.editing_text(), "Root::A long middle directory::Leaf");
    }

    #[test]
    fn path_editing_commits_the_typed_text() {
        use std::sync::{Arc, Mutex};
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 320, 28));
        breadcrumb.set_segments(sample_segments());
        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&emitted);
        breadcrumb.path_edited.connect(move |text| sink.lock().unwrap().push(text.to_string()));
        breadcrumb.set_editing(true);
        assert_eq!(breadcrumb.editing_text(), "Root/Project/src");
        breadcrumb.handle_event(&Event::TextInput { text: "/x".to_string() });
        breadcrumb.handle_event(&Event::KeyPress { key: 13, modifiers: 0 });
        assert!(!breadcrumb.is_editing());
        assert_eq!(*emitted.lock().unwrap(), vec!["Root/Project/src/x".to_string()]);
    }

    #[test]
    fn a_context_menu_names_the_segment_under_the_pointer() {
        use std::sync::{Arc, Mutex};
        let mut breadcrumb = Breadcrumb::new(Rect::new(0, 0, 320, 28));
        breadcrumb.set_segments(sample_segments());
        let asked = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&asked);
        breadcrumb
            .context_menu_requested
            .connect(move |id| sink.lock().unwrap().push(id.to_string()));
        let rect = breadcrumb.segment_rect_at(1).expect("the second segment is laid out");
        assert!(breadcrumb.context_menu_event(Point::new(rect.x + 2, rect.y + 2)));
        assert_eq!(*asked.lock().unwrap(), vec!["project".to_string()]);
    }
}
