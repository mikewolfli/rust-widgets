// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Font combo box — a text field whose trailing indicator opens a family list.
//!
//! # Why the indicator drives the value's right inset
//!
//! The indicator is the field's trailing chrome, and the value's box must *yield* to it.
//! The shared relation writes this as `rightPadding: padding + indicator.width`, and the
//! reason is that a fixed text inset and a fixed indicator inset are two unrelated
//! derivations from the *same* edge. The value used to be written at `g.x + 4` with no bound
//! while the indicator was placed from `g.width - 14`, so a long family name — `Noto Sans
//! CJK JP` in a narrow format bar — was painted underneath the arrow.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect, Size};
use crate::event::{Event, EventHandler};

use crate::signal::{GenericSignal, Signal1};
use crate::style::EdgeOffsets;
use crate::widget::capability::coercion::{
    expect_bool, expect_horizontal_alignment, expect_i32_saturating, expect_string,
    horizontal_alignment_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{
    dimensions, effective_font, focus_ring_color, ControlMetrics, FocusRing, FOCUS_RING_WIDTH,
};
use crate::widget::text_utils::{floor_char_boundary, next_char_boundary, previous_char_boundary};
use crate::widget::{BaseWidget, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// The field's own horizontal content inset: 4.
///
/// The value was written at `g.x + 4`, which is this number — named so the hint and the paint
/// read the same fact rather than one of them carrying a bare literal.
const FONT_FIELD_PADDING: i32 = 4;

/// Space between the indicator's leading edge and the end of the value's box: 2.
const INDICATOR_LEADING_GAP: u32 = 2;

/// The indicator's box and the box the value may occupy, derived together.
///
/// # Why the two are computed together
///
/// The `▼` and the family name beside it are one row, and the name must *yield* to the
/// indicator rather than be clipped by an inset chosen independently of it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct FontFieldGeometry {
    /// The indicator's bounding box at the field's trailing edge.
    indicator_box: Rect,
    /// The rectangle the family name may occupy.
    value_box: Rect,
}

impl FontFieldGeometry {
    /// Derives both boxes from the band the control paints and `line_height`.
    fn for_band(band: Rect, line_height: u32) -> Self {
        let indicator_width = dimensions::BUTTON_ICON_SIZE.min(band.width);
        let height = line_height.min(band.height);
        let indicator_x = band.x
            + band.width.saturating_sub(indicator_width + dimensions::TEXT_FIELD_PADDING_H) as i32;
        let indicator_box = Rect::new(
            indicator_x,
            band.y + (band.height.saturating_sub(height) / 2) as i32,
            indicator_width,
            height,
        );
        // The value's box stops one gap before the indicator. `min` with the leading inset keeps
        // a squeezed field from describing an inverted rectangle: a field with no room for a name
        // draws none rather than one outside itself.
        let value_right = indicator_box.x.saturating_sub(INDICATOR_LEADING_GAP as i32);
        let value_left = (band.x + FONT_FIELD_PADDING).min(value_right);
        let value_box = Rect::new(
            value_left,
            band.y,
            value_right.saturating_sub(value_left) as u32,
            band.height,
        );
        Self { indicator_box, value_box }
    }
}
/// Font combo box widget for font selection.
///
/// Holds a list of font family names and an index into it. The index uses `-1`
/// as an explicit "nothing selected" sentinel, which is why selection is an
/// `i32` rather than a `usize`.
///
/// The widget does not enumerate installed fonts: the caller populates the list
/// with [`FontComboBox::add_font`].
///
/// Filters that narrow the font list to families matching a style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FontFilters(u8);

impl FontFilters {
    /// No filtering: every family is listed.
    pub const NONE: Self = Self(0);
    /// Families whose name marks them as monospaced.
    pub const MONOSPACED: Self = Self(1);
    /// Families that are scalable outlines (all fonts the crate ships are).
    pub const SCALABLE: Self = Self(2);
    /// Families that are proportional (not monospaced).
    pub const PROPORTIONAL: Self = Self(4);

    /// Whether every filter in `other` is enabled here.
    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// The union of two filter sets.
    pub fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Whether a family name passes the monospaced heuristic.
    ///
    /// The crate does not carry per-family metadata, so a family is treated as monospaced
    /// when its name says so — the convention every platform's font menu uses. A host with
    /// real metrics can filter its own list before calling [`FontComboBox::add_font`].
    fn is_monospaced(name: &str) -> bool {
        let lower = name.to_lowercase();
        lower.contains("mono") || lower.contains("courier") || lower.contains("consolas")
    }

    /// Whether a family name marks a scalable outline face.
    ///
    /// The crate ships outline faces everywhere, but a host may append a platform bitmap
    /// family. The name heuristic is the only per-family fact available here, so a family
    /// whose name says it is raster-only fails the filter (I-118).
    fn is_scalable(name: &str) -> bool {
        let lower = name.to_lowercase();
        !lower.contains("bitmap") && !lower.contains("pixel") && !lower.contains("raster")
    }

    /// Whether `name` is listed under this filter set.
    pub fn matches(self, name: &str) -> bool {
        if self == Self::NONE {
            return true;
        }
        let mono = Self::is_monospaced(name);
        if self.contains(Self::MONOSPACED) && !mono {
            return false;
        }
        if self.contains(Self::SCALABLE) && !Self::is_scalable(name) {
            return false;
        }
        if self.contains(Self::PROPORTIONAL) && mono {
            return false;
        }
        true
    }

    /// The published token list naming the enabled filters, in canonical order.
    ///
    /// An empty string means [`Self::NONE`], so a caller reading the property sees exactly
    /// what it wrote without needing a second "is filtering" flag.
    pub fn to_tokens(self) -> crate::compat::String {
        let mut tokens: crate::compat::Vec<&str> = crate::compat::Vec::new();
        if self.contains(Self::MONOSPACED) {
            tokens.push("monospaced");
        }
        if self.contains(Self::SCALABLE) {
            tokens.push("scalable");
        }
        if self.contains(Self::PROPORTIONAL) {
            tokens.push("proportional");
        }
        tokens.join(",")
    }

    /// The filter set a comma-separated token list names.
    ///
    /// Unknown tokens are refused rather than ignored: a typo in a property editor should
    /// report itself, not silently list every font.
    pub fn from_tokens(text: &str) -> Option<Self> {
        let mut filters = Self::NONE;
        for token in text.split(',').map(str::trim).filter(|token| !token.is_empty()) {
            let filter = match token {
                "monospaced" => Self::MONOSPACED,
                "scalable" => Self::SCALABLE,
                "proportional" => Self::PROPORTIONAL,
                _ => return None,
            };
            filters = filters.union(filter);
        }
        Some(filters)
    }
}

/// The ordered selection range of an editable value, in bytes (I-16).
fn edit_selection(anchor: Option<usize>, cursor: usize) -> (usize, usize) {
    match anchor {
        Some(anchor) if anchor != cursor => (anchor.min(cursor), anchor.max(cursor)),
        _ => (cursor, cursor),
    }
}

/// A combo box whose entries are font family names.
pub struct FontComboBox {
    base: BaseWidget,
    current_font: Font,
    fonts: Vec<String>,
    /// The list after the style and writing-system filters, as family names.
    visible_fonts: Vec<String>,
    /// Style filters applied to the list.
    font_filters: FontFilters,
    /// A writing-system filter, matched against the family name when set.
    writing_system_filter: Option<String>,
    /// First painted popup row.
    scroll_offset: usize,
    /// Prefix typed since the last navigation, for first-letter selection.
    type_ahead: String,
    /// When the current type-ahead prefix began, for the shared 800 ms rule (I-P06).
    type_ahead_started: Option<crate::compat::Instant>,
    /// Original indices of the rows in [`Self::visible_fonts`], parallel to it.
    ///
    /// The paint and the hit test need the row's index in the *unfiltered* list, and searching
    /// for it per painted row made a menu of n fonts cost O(visible x n) per frame (I-P08).
    /// Rebuilt with the filtered list, which is the only time the mapping can change.
    visible_indices: Vec<usize>,
    /// Bumped whenever the filtered list is rebuilt, so the row-font cache cannot outlive it.
    visible_revision: u64,
    /// One preview font per visible row, rebuilt when the list or the row size changes (I-P08).
    ///
    /// Every painted row used to build `Font::simple(name.clone(), 14.0)` per frame — one
    /// family string allocation and one face lookup per row per frame while the list was open.
    popup_fonts: core::cell::RefCell<Option<(u64, u32, Vec<Font>)>>,
    /// Optional sample text painted beside each family name in the popup.
    sample_text: Option<String>,
    current_index: i32,
    editable: bool,
    /// In-progress typed text while the box is editable.
    ///
    /// `None` means "not editing": the field shows the current font instead. It is
    /// distinct from `Some(String::new())`, which is an edit in progress whose text
    /// the user has erased — the two render differently, and collapsing them would
    /// make a cleared field look like an idle one.
    edit_buffer: Option<String>,
    /// The edit's caret, in bytes from the start of the buffer (I-16).
    edit_cursor: usize,
    /// The other end of the edit's selection, in bytes. `None` means no selection (I-16).
    edit_anchor: Option<usize>,
    /// The editable caret's blink phase (X-003); advanced only while an edit is in progress.
    cursor_blink: crate::style::CursorBlink,
    max_visible_items: i32,
    expanded: bool,
    /// How the field's value is aligned within its own box.
    ///
    /// Horizontal only: the family name is centred vertically in the field, so a `top`/`bottom`
    /// value would be one this control could never honour —
    /// [`crate::widget::capability::coercion::expect_horizontal_alignment`] refuses those rather
    /// than accepting a write that does nothing. Defaults to left.
    alignment: crate::core::Alignment,
    /// Emitted with the new font whenever [`FontComboBox::set_current_font`]
    /// actually changes it — including as a side effect of changing the index.
    pub current_font_changed: Signal1<Font>,
    /// Emitted with the new index (possibly `-1`) whenever
    /// [`FontComboBox::set_current_index`] actually changes it.
    pub current_index_changed: Signal1<i32>,
    /// Emitted with the index that was activated by the user. Currently only
    /// fired from the mouse-release path, with the same index that was just
    /// passed to `set_current_index`.
    pub activated: Signal1<i32>,
    /// Emitted when the user commits typed text while the combo box is editable.
    ///
    /// The payload is the committed family name. "Committed" rather than "every
    /// keystroke": a font family is only meaningful once the caller acts on it, and
    /// firing per keystroke would make a handler that loads a font reload it on
    /// every character. Enter and the popup's own selection both commit.
    pub text_edited: Signal1<String>,
    /// Emitted when the popup list is opened.
    pub popup_shown: GenericSignal,
    /// Emitted when the popup list is closed.
    pub popup_hidden: GenericSignal,
    /// Whether recently chosen families move to the top of the list (I-119).
    mru_enabled: bool,
    /// The existing MRU store, reused rather than reimplemented (COMP-B03).
    recent_fonts: super::recent_values_popup::RecentValuesPopup,
}
impl FontComboBox {
    /// The band the control actually paints: full width, one field tall, centred.
    ///
    /// # Why the field is not the control's rectangle
    ///
    /// A font combo box is a text field with an indicator in it, and
    /// [`dimensions::TEXT_FIELD_MIN_HEIGHT`] is what every field in this crate occupies. The
    /// 240x120 census cell drew a 240x120 slab, so the font chooser and the `line_edit` beside
    /// it in a format bar were different objects. The band is the single derivation the fill,
    /// the border, the indicator and the value's box all read.
    fn field_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::TEXT_FIELD_MIN_HEIGHT)
    }

    /// The indicator's box and the value's box, derived from the band and one line height.
    fn field_geometry(&self, line_height: u32) -> FontFieldGeometry {
        FontFieldGeometry::for_band(self.field_band(), line_height)
    }

    /// Creates an empty, non-editable combo box with no selection
    /// ([`FontComboBox::current_index`] `-1`), the default font, and a maximum
    /// of 10 visible popup items (the popup starts closed).
    ///
    /// `geometry` is in parent-relative logical pixels; the size hint is
    /// 200x28.
    pub fn new(geometry: Rect) -> Self {
        let default_font = Font::default();
        Self {
            base: BaseWidget::new(WidgetKind::FontComboBox, geometry, "FontComboBox"),
            current_font: default_font.clone(),
            fonts: Vec::new(),
            visible_fonts: Vec::new(),
            font_filters: FontFilters::NONE,
            writing_system_filter: None,
            scroll_offset: 0,
            type_ahead: String::new(),
            type_ahead_started: None,
            visible_indices: Vec::new(),
            visible_revision: 0,
            popup_fonts: core::cell::RefCell::new(None),
            sample_text: None,
            current_index: -1,
            editable: false,
            edit_buffer: None,
            edit_cursor: 0,
            edit_anchor: None,
            cursor_blink: crate::style::CursorBlink::new(),
            max_visible_items: 10,
            expanded: false,
            alignment: crate::core::Alignment::Left,
            current_font_changed: Signal1::new(),
            current_index_changed: Signal1::new(),
            activated: Signal1::new(),
            text_edited: Signal1::new(),
            popup_shown: GenericSignal::new(),
            popup_hidden: GenericSignal::new(),
            mru_enabled: false,
            recent_fonts: super::recent_values_popup::RecentValuesPopup::new(Rect::new(0, 0, 0, 0)),
        }
    }

    /// Compatibility spelling of [`Self::current_index_changed`] (W-20).
    ///
    /// Both names resolve to the same emitted signal; the published row keeps the
    /// index spelling, so this accessor is the only Rust-side canonical name.
    pub fn current_changed(&self) -> &Signal1<i32> {
        &self.current_index_changed
    }
    /// Returns the font family and size currently in effect. This is a
    /// [`Font`] value, which carries the size; note that selecting an index
    /// keeps the size but resets the bold/italic flags to `false`.
    pub fn current_font(&self) -> &Font {
        &self.current_font
    }
    /// Returns the list of selectable family names, in display order.
    pub fn fonts(&self) -> &[String] {
        &self.fonts
    }
    /// Returns the selected index, or `None` when nothing is selected.
    ///
    /// Canonical spelling shared with the other selection controls (WDGT-13); the old `-1`
    /// sentinel is available through the deprecated [`Self::current_index`].
    pub fn selected_index(&self) -> Option<usize> {
        (self.current_index >= 0).then_some(self.current_index as usize)
    }

    /// Returns the selected index, or `-1` when nothing is selected.
    #[deprecated(note = "use `selected_index` (Option) — WDGT-13")]
    pub fn current_index(&self) -> i32 {
        self.current_index
    }
    /// Returns whether the combo box is marked editable. Defaults to `false`.
    ///
    /// When editable, typed characters accumulate in [`Self::edit_buffer`], Enter or
    /// [`Self::commit_edit`] emits [`Self::text_edited`], and Escape discards.
    pub fn is_editable(&self) -> bool {
        self.editable
    }
    /// Returns how many popup entries are shown before the list is truncated.
    /// Always at least `1`; defaults to `10`.
    pub fn max_visible_items(&self) -> i32 {
        self.max_visible_items
    }

    /// How the field's value is aligned within its own box.
    pub fn alignment(&self) -> crate::core::Alignment {
        self.alignment
    }

    /// Sets how the field's value is aligned within its own box.
    ///
    /// Horizontal only. A `top`/`bottom` alignment is **ignored**, because the family name is
    /// centred vertically in the field by the field's own layout — the property route refuses it
    /// through [`crate::widget::capability::coercion::expect_horizontal_alignment`], and this
    /// setter matching that keeps the two entry points from disagreeing.
    pub fn set_alignment(&mut self, alignment: crate::core::Alignment) {
        if alignment.to_horizontal().is_none() || self.alignment == alignment {
            return;
        }
        self.alignment = alignment;
        self.base.request_redraw();
    }
    /// Returns the number of fonts in the list; equivalent to `fonts().len()`
    /// narrowed to `i32`.
    pub fn count(&self) -> i32 {
        self.fonts.len() as i32
    }
    /// Replaces the effective font.
    ///
    /// A no-op when the value is unchanged: no signal, no redraw. This does
    /// **not** update [`FontComboBox::current_index`], so the index and the font
    /// can disagree until the index is set. Emits `current_font_changed`.
    pub fn set_current_font(&mut self, font: Font) {
        if self.current_font != font {
            self.current_font = font.clone();
            self.current_font_changed.emit(font);
            self.base.request_redraw();
        }
    }
    /// Selects an entry, clamped to `-1 ..= count()-1`.
    ///
    /// A no-op when the clamped index is unchanged. Otherwise emits
    /// `current_index_changed`, and for a non-negative index also derives the
    /// font from the selected name — **keeping the current point size but
    /// clearing the bold and italic flags** — which may in turn emit
    /// `current_font_changed`. Index `-1`, or an empty list (where the clamp is
    /// also `-1`), leaves the font untouched.
    pub fn set_current_index(&mut self, index: i32) {
        let clamped = index.clamp(-1, self.fonts.len() as i32 - 1);
        if self.current_index != clamped {
            self.current_index = clamped;
            self.current_index_changed.emit(clamped);
            if clamped >= 0 && clamped < self.fonts.len() as i32 {
                // Update current font based on selection
                if let Some(font_name) = self.fonts.get(clamped as usize) {
                    let new_font = Font::new(font_name, self.current_font.size(), false, false);
                    if self.mru_enabled {
                        // The MRU store is the same one the recent-values popup uses, so the
                        // reorder rule has one implementation (I-119).
                        self.recent_fonts.remember(font_name);
                        self.refresh_visible_fonts();
                    }
                    self.set_current_font(new_font);
                }
            }
            self.base.request_redraw();
        }
    }
    /// Sets the editable flag. See [`FontComboBox::is_editable`] — it currently
    /// has no behavioural effect beyond being reported. Requests a redraw.
    pub fn set_editable(&mut self, editable: bool) {
        if self.editable == editable {
            return;
        }
        self.editable = editable;
        if !editable {
            // Turning the feature off abandons any in-progress edit, so the field
            // cannot keep showing text a non-editable box would not accept.
            self.edit_buffer = None;
            self.edit_cursor = 0;
            self.edit_anchor = None;
        }
        self.base.request_redraw();
    }
    /// The in-progress typed text, or `None` when no edit is under way.
    ///
    /// `Some("")` is a real state: an edit the user has erased. See the field's own
    /// comment for why the two are not collapsed.
    pub fn edit_buffer(&self) -> Option<&str> {
        self.edit_buffer.as_deref()
    }
    /// Commits the typed text, emitting `text_edited`.
    ///
    /// Committing an empty edit is refused rather than emitting an empty family
    /// name: an empty string is not a font, and a caller that loads what the signal
    /// names would have nothing to load. Returns `true` when a commit happened.
    pub fn commit_edit(&mut self) -> bool {
        let Some(text) = self.edit_buffer.take() else {
            return false;
        };
        // The edit is over, so the caret and the range it described go with it (I-16).
        self.edit_cursor = 0;
        self.edit_anchor = None;
        self.cursor_blink.stop();
        self.base.request_redraw();
        if text.trim().is_empty() {
            return false;
        }
        self.text_edited.emit(text.clone());
        // A typed family that matches a listed font also moves the selection, so the
        // typed path and the picker path cannot end up describing different fonts.
        if let Some(index) = self.fonts.iter().position(|family| family == &text) {
            self.set_current_index(index as i32);
        }
        true
    }

    /// The edit's caret, in bytes from the start of the buffer (I-16).
    pub fn edit_cursor_position(&self) -> usize {
        self.edit_cursor
    }

    /// The edit's selected byte range, or `None` when nothing is selected (I-16).
    pub fn edit_selection_range(&self) -> Option<(usize, usize)> {
        let anchor = self.edit_anchor?;
        let (start, end) = edit_selection(Some(anchor), self.edit_cursor);
        (start != end).then_some((start, end))
    }

    /// Starts an edit at the end of the current value when none is under way (I-16).
    fn begin_edit(&mut self) {
        if self.edit_buffer.is_none() {
            let seed = self.current_font.family().to_string();
            self.edit_cursor = seed.len();
            self.edit_anchor = None;
            self.edit_buffer = Some(seed);
            self.cursor_blink.start();
        }
    }

    /// Replaces the edit's selection with `text`, or inserts at the caret (I-16).
    fn edit_insert(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        self.begin_edit();
        let (start, end) = edit_selection(self.edit_anchor, self.edit_cursor);
        let (start, end) = match self.edit_buffer.as_ref() {
            Some(buffer) => (
                floor_char_boundary(buffer, start.min(buffer.len())),
                floor_char_boundary(buffer, end.min(buffer.len())),
            ),
            None => return,
        };
        if let Some(buffer) = self.edit_buffer.as_mut() {
            buffer.replace_range(start..end, text);
        }
        self.edit_cursor = start + text.len();
        self.edit_anchor = None;
        self.base.request_redraw();
    }

    /// Deletes the edit's selection, or the character before the caret (I-16).
    fn edit_backspace(&mut self) {
        self.begin_edit();
        let (start, end) = match self.edit_buffer.as_ref() {
            Some(buffer) => {
                let (start, end) = edit_selection(self.edit_anchor, self.edit_cursor);
                if start != end {
                    (
                        floor_char_boundary(buffer, start.min(buffer.len())),
                        floor_char_boundary(buffer, end.min(buffer.len())),
                    )
                } else {
                    let caret = floor_char_boundary(buffer, self.edit_cursor.min(buffer.len()));
                    (previous_char_boundary(buffer, caret), caret)
                }
            }
            None => return,
        };
        if start == end {
            return;
        }
        if let Some(buffer) = self.edit_buffer.as_mut() {
            buffer.replace_range(start..end, "");
        }
        self.edit_cursor = start;
        self.edit_anchor = None;
        self.base.request_redraw();
    }

    /// Deletes the edit's selection, or the character after the caret (I-16).
    fn edit_delete_forward(&mut self) {
        self.begin_edit();
        let (start, end) = match self.edit_buffer.as_ref() {
            Some(buffer) => {
                let (start, end) = edit_selection(self.edit_anchor, self.edit_cursor);
                if start != end {
                    (
                        floor_char_boundary(buffer, start.min(buffer.len())),
                        floor_char_boundary(buffer, end.min(buffer.len())),
                    )
                } else {
                    let caret = floor_char_boundary(buffer, self.edit_cursor.min(buffer.len()));
                    (caret, next_char_boundary(buffer, caret))
                }
            }
            None => return,
        };
        if start == end {
            return;
        }
        if let Some(buffer) = self.edit_buffer.as_mut() {
            buffer.replace_range(start..end, "");
        }
        self.edit_cursor = start;
        self.edit_anchor = None;
        self.base.request_redraw();
    }

    /// Moves the edit's caret by `delta` characters, extending the range when asked (I-16).
    fn edit_move_caret(&mut self, delta: i32, extend: bool) {
        self.begin_edit();
        let target = match self.edit_buffer.as_ref() {
            Some(buffer) => {
                let mut target = self.edit_cursor.min(buffer.len());
                if delta < 0 {
                    for _ in 0..delta.unsigned_abs() {
                        target = previous_char_boundary(buffer, target);
                    }
                } else {
                    for _ in 0..delta.unsigned_abs() {
                        target = next_char_boundary(buffer, target);
                    }
                }
                floor_char_boundary(buffer, target)
            }
            None => return,
        };
        self.edit_set_caret(target, extend);
    }

    /// Places the edit's caret, extending the range when asked (I-16).
    fn edit_set_caret(&mut self, position: usize, extend: bool) {
        self.begin_edit();
        let target = match self.edit_buffer.as_ref() {
            Some(buffer) => floor_char_boundary(buffer, position.min(buffer.len())),
            None => return,
        };
        if extend {
            if self.edit_anchor.is_none() {
                self.edit_anchor = Some(self.edit_cursor);
            }
        } else {
            self.edit_anchor = None;
        }
        self.edit_cursor = target;
        self.base.request_redraw();
    }
    /// Sets how many popup entries are shown, floored at `1` so the popup is
    /// never zero-height. Does not request a redraw (unlike the other setters),
    /// so an already-open popup may not repaint until the next redraw.
    pub fn set_max_visible_items(&mut self, max_items: i32) {
        let clamped = max_items.max(1);
        if self.max_visible_items == clamped {
            return;
        }
        self.max_visible_items = clamped;
        // The list's height is derived from this, so a popup that is open has to be re-laid-out.
        self.base.request_redraw();
    }

    /// The style filters applied to the font list.
    pub fn font_filters(&self) -> FontFilters {
        self.font_filters
    }

    /// Applies style filters to the font list.
    pub fn set_font_filters(&mut self, filters: FontFilters) {
        if self.font_filters == filters {
            return;
        }
        self.font_filters = filters;
        self.refresh_visible_fonts();
        self.base.request_redraw();
    }

    /// The writing-system filter, if one is set.
    pub fn writing_system_filter(&self) -> Option<&str> {
        self.writing_system_filter.as_deref()
    }

    /// Restricts the list to families whose name matches `filter`.
    ///
    /// The crate carries no per-family script metadata, so the match is against the name
    /// (`"CJK"`, `"Arabic"`, …). A host with real coverage data filters its own list.
    pub fn set_writing_system_filter(&mut self, filter: Option<String>) {
        if self.writing_system_filter == filter {
            return;
        }
        self.writing_system_filter = filter;
        self.refresh_visible_fonts();
        self.base.request_redraw();
    }

    /// The families currently listed, after filtering.
    pub fn visible_fonts(&self) -> &[String] {
        &self.visible_fonts
    }

    /// The sample text painted beside each family in the popup, if any.
    pub fn sample_text(&self) -> Option<&str> {
        self.sample_text.as_deref()
    }

    /// Sets the sample text painted beside each family in the popup.
    ///
    /// `None` paints only the family name, which is the historical frame. A sample is the
    /// only way a font menu shows what a family looks like in more than its own name.
    pub fn set_sample_text(&mut self, sample: Option<String>) {
        if self.sample_text == sample {
            return;
        }
        self.sample_text = sample;
        self.base.request_redraw();
    }

    /// The first painted popup row.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Sets the first painted popup row, clamped to the rows that exist.
    pub fn set_scroll_offset(&mut self, offset: usize) {
        let rows = self.visible_fonts.len();
        let page = self.page_size(rows);
        let next = offset.min(rows.saturating_sub(page.min(rows)));
        if self.scroll_offset == next {
            return;
        }
        self.scroll_offset = next;
        self.base.request_redraw();
    }

    /// Number of popup rows painted at once.
    pub fn page_size(&self, rows: usize) -> usize {
        (self.max_visible_items.max(1) as usize).min(rows.max(1))
    }

    /// Recomputes the filtered list from the filters and the source families.
    fn refresh_visible_fonts(&mut self) {
        // The writing-system filter is folded **once** per rebuild and every family compared
        // byte-wise, instead of folding every family (I-P05).
        let writing = self
            .writing_system_filter
            .as_ref()
            .map(|filter| crate::widget::input_widgets::text_match::fold(filter));
        let accepts = |filters: FontFilters, name: &str, writing: Option<&String>| {
            filters.matches(name)
                && writing.is_none_or(|filter| {
                    filter.is_empty()
                        || crate::widget::input_widgets::text_match::contains(name, filter)
                })
        };
        self.visible_fonts.clear();
        self.visible_indices.clear();
        // Recently chosen families lead the list when the option is on. `seen` keeps the
        // reorder linear and prevents a family from being listed twice (I-119).
        let mut seen = vec![false; self.fonts.len()];
        if self.mru_enabled {
            for recent in self.recent_fonts.entries() {
                let Some(index) = self.fonts.iter().position(|name| name == recent) else {
                    continue;
                };
                if seen[index] || !accepts(self.font_filters, &self.fonts[index], writing.as_ref())
                {
                    continue;
                }
                seen[index] = true;
                self.visible_fonts.push(self.fonts[index].clone());
                self.visible_indices.push(index);
            }
        }
        for (index, name) in self.fonts.iter().enumerate() {
            if seen[index] || !accepts(self.font_filters, name, writing.as_ref()) {
                continue;
            }
            self.visible_fonts.push(name.clone());
            self.visible_indices.push(index);
        }
        self.visible_revision = self.visible_revision.wrapping_add(1);
        self.scroll_offset = 0;
    }

    /// The original index of a family name, if it is listed.
    fn original_index_of(&self, name: &str) -> Option<usize> {
        self.fonts.iter().position(|family| family == name)
    }

    /// The original index of the row at `row` in the filtered list.
    fn original_index_at(&self, row: usize) -> Option<usize> {
        self.visible_indices.get(row).copied()
    }

    /// The size the popup rows are drawn at: the field's **effective** font size (I-15).
    fn popup_row_font_size(&self) -> f32 {
        effective_font(self.base.style()).size().max(1.0)
    }

    /// The height of one popup row: the row font's size plus eight pixels of leading (I-15).
    ///
    /// One derivation, read by both the paint and [`Self::popup_row_at`], so a larger text
    /// preference grows the rows *and* moves the hit test with them.
    fn popup_row_height(&self) -> i32 {
        (self.popup_row_font_size() + 8.0) as i32
    }

    /// Runs `f` with one preview font per visible row, rebuilding the cache when it is stale.
    fn with_popup_fonts<R>(&self, size: f32, f: impl FnOnce(&[Font]) -> R) -> R {
        if let Ok(mut cache) = self.popup_fonts.try_borrow_mut() {
            let stale = cache.as_ref().is_none_or(|(revision, cached_size, fonts)| {
                *revision != self.visible_revision
                    || *cached_size != size.to_bits()
                    || fonts.len() != self.visible_fonts.len()
            });
            if stale {
                let fonts: Vec<Font> = self
                    .visible_fonts
                    .iter()
                    .map(|name| Font::simple(name.clone(), size))
                    .collect();
                *cache = Some((self.visible_revision, size.to_bits(), fonts));
            }
            if let Some((_, _, fonts)) = cache.as_ref() {
                return f(fonts.as_slice());
            }
        }
        let fonts: Vec<Font> =
            self.visible_fonts.iter().map(|name| Font::simple(name.clone(), size)).collect();
        f(&fonts)
    }

    /// The popup row a point falls on.
    fn popup_row_at(&self, pos: Point) -> Option<usize> {
        if !self.expanded || self.visible_fonts.is_empty() {
            return None;
        }
        let band = self.field_band();
        let popup_top = band.y + band.height as i32;
        if pos.x < band.x || pos.x >= band.x + band.width as i32 || pos.y < popup_top {
            return None;
        }
        let row =
            ((pos.y - popup_top) / self.popup_row_height().max(1)) as usize + self.scroll_offset;
        (row < self.visible_fonts.len()).then_some(row)
    }
    /// Appends a family name to the end of the list and requests a redraw.
    ///
    /// Duplicates are not filtered: adding the same name twice yields two
    /// identical, independently selectable entries.
    pub fn add_font(&mut self, font_name: String) {
        self.fonts.push(font_name);
        self.refresh_visible_fonts();
        self.base.request_redraw();
    }
    /// Replaces the family list with the host's installed families, or the families compiled
    /// into this build when the backend reports none.
    ///
    /// Returns the number of entries now offered. The list comes from the process-wide snapshot
    /// in [`crate::platform::fonts`], so opening several controls does not repeat the OS query;
    /// a caller that has just installed a font refreshes that snapshot first. The selection
    /// becomes the current font's family when the list contains it, and `-1` otherwise.
    pub fn load_system_fonts(&mut self) -> usize {
        self.fonts = crate::platform::fonts::chooser_families();
        let family = self.current_font.family();
        self.current_index = self
            .fonts
            .iter()
            .position(|candidate| candidate.eq_ignore_ascii_case(family))
            .map_or(-1, |index| index as i32);
        self.refresh_visible_fonts();
        self.base.request_redraw();
        self.fonts.len()
    }
    /// Removes the font at `index`; out-of-range indices are ignored.
    ///
    /// If the removed entry was selected, the selection is cleared to `-1`;
    /// otherwise an index after the removal point is shifted down by one so it
    /// keeps referring to the same entry. Note that only the raw index field is
    /// adjusted in that case — `current_index_changed` is **not** emitted —
    /// though the selection still points at the same font.
    pub fn remove_font(&mut self, index: i32) {
        if index >= 0 && index < self.fonts.len() as i32 {
            self.fonts.remove(index as usize);
            if self.current_index == index {
                self.set_current_index(-1);
            } else if self.current_index > index {
                self.current_index -= 1;
            }
            self.refresh_visible_fonts();
            self.base.request_redraw();
        }
    }
    /// Empties the font list and clears the selection via
    /// [`FontComboBox::set_current_index`], so `current_index_changed` fires if
    /// an entry had been selected.
    pub fn clear(&mut self) {
        self.fonts.clear();
        self.visible_fonts.clear();
        self.scroll_offset = 0;
        self.set_current_index(-1);
        self.base.request_redraw();
    }
    /// Opens the popup list, emits `popup_shown`, and requests a redraw.
    ///
    /// Not idempotent: calling it while already open emits again. The widget
    /// does not close the popup on outside clicks.
    pub fn show_popup(&mut self) {
        self.expanded = true;
        self.popup_shown.emit();
        self.base.request_redraw();
    }
    /// Closes the popup list, emits `popup_hidden`, and requests a redraw.
    /// As with [`FontComboBox::show_popup`], calling it while already closed
    /// still emits.
    pub fn hide_popup(&mut self) {
        self.expanded = false;
        self.popup_hidden.emit();
        self.base.request_redraw();
    }

    /// Whether recently chosen families are moved to the top of the popup.
    pub fn mru_enabled(&self) -> bool {
        self.mru_enabled
    }

    /// Turns the recently-used ordering on or off; off by default (I-119).
    pub fn set_mru_enabled(&mut self, enabled: bool) {
        if self.mru_enabled == enabled {
            return;
        }
        self.mru_enabled = enabled;
        self.refresh_visible_fonts();
        self.base.request_redraw();
    }

    /// The recently chosen families, newest first.
    pub fn recent_fonts(&self) -> &[String] {
        self.recent_fonts.entries()
    }
    /// Returns the family name of the selected entry, or an empty string when
    /// nothing is selected ([`FontComboBox::current_index`] `-1`).
    pub fn current_text(&self) -> String {
        if self.current_index >= 0 && self.current_index < self.fonts.len() as i32 {
            self.fonts[self.current_index as usize].clone()
        } else {
            String::new()
        }
    }
}
impl Widget for FontComboBox {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    fn tick(&mut self, delta_ms: u32) -> bool {
        if self.edit_buffer.is_none() {
            self.cursor_blink.stop();
            return false;
        }
        let running = self.cursor_blink.tick(delta_ms);
        if running {
            self.base.request_redraw();
        }
        running
    }

    fn is_animating(&self) -> bool {
        self.expanded || self.edit_buffer.is_some()
    }

    fn has_open_popup(&self) -> bool {
        self.expanded
    }

    /// Reports the edit caret so an IME candidate window anchors to it (X-001).
    fn input_method_query(&self) -> Option<crate::widget::widget_trait::InputMethodQuery> {
        let text = self.edit_buffer.as_ref()?;
        let font = effective_font(self.style());
        let line_height = font.effective_line_height().max(1.0) as u32;
        let geometry = self.field_geometry(line_height);
        let caret_byte = floor_char_boundary(text, self.edit_cursor.min(text.len()));
        let advance =
            crate::widget::metrics::estimate_text_width(&text[..caret_byte], font, 1.0) as i32;
        let align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
        let total = crate::widget::metrics::estimate_text_width(text, font, 1.0) as i32;
        let origin = crate::render::fitted_origin(
            Rect::new(
                geometry.value_box.x,
                geometry.value_box.y,
                geometry.value_box.width,
                geometry.value_box.height,
            ),
            total,
            align,
        );
        let x = (origin.x + advance)
            .min(geometry.value_box.x + geometry.value_box.width as i32 - 1)
            .max(geometry.value_box.x);
        Some(crate::widget::widget_trait::InputMethodQuery {
            caret_rect: Rect::new(x, geometry.value_box.y, 1, geometry.value_box.height.max(1)),
            caret_byte: Some(caret_byte),
        })
    }

    fn size_hint(&self) -> crate::core::Size {
        // Derived through the same metric the band is drawn from, so the reported size and the
        // painted box cannot describe two different controls. The width was a bare `200` and the
        // height `28` while `draw` painted the whole given rectangle — 120 px tall in the
        // census cell, i.e. four times the height this reports.
        let side_air = (dimensions::TEXT_FIELD_MIN_HEIGHT / 2).saturating_sub(8);
        let trailing =
            (FONT_FIELD_PADDING as u32) + dimensions::BUTTON_ICON_SIZE + INDICATOR_LEADING_GAP;
        let padding = EdgeOffsets {
            top: side_air,
            right: trailing,
            bottom: side_air,
            left: FONT_FIELD_PADDING as u32,
        };
        let floor = Size::new(
            FONT_FIELD_PADDING as u32 + dimensions::BUTTON_ICON_SIZE + trailing,
            dimensions::TEXT_FIELD_MIN_HEIGHT,
        );
        let hint = ControlMetrics::implicit_size(Size::new(0, 0), padding, floor);
        Size::new(hint.width, self.field_band().height.max(floor.height))
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
    ///
    /// # Payload mapping
    ///
    /// A hub name carries no value, so a `Signal1<T>` needs a conversion. The one used here is the
    /// same spelling the property side uses, so a designer reads one representation per kind: a
    /// number as `Int`, a float as `Float`, a flag as `Bool`, text as `String`, and a structured
    /// payload as its debug spelling. A `None` maps to `Null`, which is the same value an absent
    /// optional property uses — so "no selection" is distinguishable from "selection 0".
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "current_font_changed" => Some(EventSignalRef::mapped(
                "current_font_changed",
                &self.current_font_changed,
                |v| CapabilityValue::String(format!("{v:?}")),
            )),
            "current_index_changed" => Some(EventSignalRef::mapped(
                "current_index_changed",
                &self.current_index_changed,
                |v| CapabilityValue::Int(*v as i64),
            )),
            "activated" => Some(EventSignalRef::mapped("activated", &self.activated, |v| {
                CapabilityValue::Int(*v as i64)
            })),
            "text_edited" => Some(EventSignalRef::mapped("text_edited", &self.text_edited, |v| {
                CapabilityValue::String(v.clone())
            })),
            "popup_shown" => Some(EventSignalRef::unit("popup_shown", &self.popup_shown)),
            "popup_hidden" => Some(EventSignalRef::unit("popup_hidden", &self.popup_hidden)),
            _ => None,
        }
    }
}

/// `FontComboBox`'s property contract.
///
/// This control uses `i32` indices (the `-1` sentinel means "no font"), which is
/// why its index properties are `Int` rather than `UInt` — the split
/// `FONT_COMBO_BOX_PROPERTIES` records and the old dispatch preserved.
impl WidgetProperties for FontComboBox {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "current_font_family" => Ok(CapabilityValue::String(self.current_text())),
            "item_count" => Ok(CapabilityValue::Int(self.count() as i64)),
            "current_index" => Ok(CapabilityValue::Int(
                self.selected_index().map(|index| index as i32).unwrap_or(-1) as i64,
            )),
            "editable" => Ok(CapabilityValue::Bool(self.is_editable())),
            "alignment" => Ok(CapabilityValue::String(
                horizontal_alignment_to_str(self.alignment()).to_string(),
            )),
            "max_visible_items" => Ok(CapabilityValue::Int(self.max_visible_items() as i64)),
            "font_filters" => Ok(CapabilityValue::String(self.font_filters().to_tokens())),
            "writing_system_filter" => match self.writing_system_filter() {
                Some(filter) => Ok(CapabilityValue::String(filter.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            "sample_text" => match self.sample_text() {
                Some(sample) => Ok(CapabilityValue::String(sample.to_string())),
                None => Ok(CapabilityValue::Null),
            },
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => {
                self.set_current_index(expect_i32_saturating(value)?);
                Ok(())
            }
            "editable" => {
                self.set_editable(expect_bool(value)?);
                Ok(())
            }
            "max_visible_items" => {
                self.set_max_visible_items(expect_i32_saturating(value)?);
                Ok(())
            }
            "alignment" => {
                self.set_alignment(expect_horizontal_alignment(value)?);
                Ok(())
            }
            "font_filters" => {
                let token = expect_string(value)?;
                let filters =
                    FontFilters::from_tokens(&token).ok_or(CapabilityAccessError::OutOfRange)?;
                self.set_font_filters(filters);
                Ok(())
            }
            "writing_system_filter" => {
                let filter = match value {
                    CapabilityValue::Null => None,
                    other => Some(expect_string(other)?),
                };
                self.set_writing_system_filter(filter);
                Ok(())
            }
            "sample_text" => {
                let sample = match value {
                    CapabilityValue::Null => None,
                    other => Some(expect_string(other)?),
                };
                self.set_sample_text(sample);
                Ok(())
            }
            // Derived from the font list.
            "current_font_family" | "item_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        // Mirrors `FONT_COMBO_BOX_PROPERTIES`.
        property_names_of![
            "current_font_family",
            "item_count",
            "current_index",
            "editable",
            "alignment",
            "max_visible_items",
            "font_filters",
            "writing_system_filter",
            "sample_text",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `font_combo_box` publishes.
    ///
    /// `show_popup` and `hide_popup` are the zero-argument actions and go through the
    /// control's own methods, so the `popup_shown` / `popup_hidden` signals are emitted
    /// exactly as they are for a mouse-driven open. They are idempotent in the sense that
    /// matters here: opening an open popup still performed the command, so it answers
    /// `Ok(())` rather than reporting a failure the caller did not cause.
    ///
    /// `set_current_index`, `set_editable` and `set_max_visible_items` assign state and
    /// need a payload, so they are answered through the property route.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "show_popup" => {
                self.show_popup();
                Ok(())
            }
            "hide_popup" => {
                self.hide_popup();
                Ok(())
            }
            "set_current_index" | "set_editable" | "set_max_visible_items" => {
                Err(CapabilityAccessError::OutOfRange)
            }
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

use crate::render::RenderContext;
use crate::widget::Draw;

impl FontComboBox {
    /// Resolves a press at `pos`: the field opens the list, a popup row commits that row, and
    /// a press outside both dismisses.
    fn press_at(&mut self, pos: Point) {
        // Arm only for a press on the control; a press outside must not leave the
        // cycle-on-release latch armed.
        let inside = self.geometry().contains_point(pos) || self.popup_row_at(pos).is_some();
        self.base.set_pressed(inside);
        if !inside {
            // A press outside the field and its popup dismisses the list, the same
            // contract every other popup in the crate has.
            self.hide_popup();
        } else if self.geometry().contains_point(pos) {
            // Show the dropdown list
            self.show_popup();
            self.base.clicked.emit();
        } else if let Some(row) = self.popup_row_at(pos) {
            // A press on a popup row commits that row directly; the row the user
            // pressed is the row they meant.
            if let Some(name) = self.visible_fonts.get(row) {
                if let Some(original) = self.original_index_of(name) {
                    self.set_current_index(original as i32);
                    self.activated.emit(original as i32);
                }
            }
            self.hide_popup();
            self.base.set_pressed(false);
        }
    }
}

impl EventHandler for FontComboBox {
    fn handle_event(&mut self, event: &Event) {
        if !self.base.is_enabled() {
            self.base.handle_event(event);
            return;
        }
        // The gesture is resolved before the base records the event: the base clears
        // `pressed` on the release, so the commit guard must read the value the press set.
        match event {
            Event::KeyPress { key, modifiers } if self.editable => {
                // Mirrors `EditableComboBox`'s key convention: 8 is backspace, 127 forward
                // delete, 13 Enter, 27 Escape, the caret keys move the insertion point (Shift
                // extends), and any other printable code is a character (I-16).
                let modifiers = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = modifiers.contains(crate::shortcut::Modifiers::SHIFT);
                match *key {
                    8 => self.edit_backspace(),
                    127 => self.edit_delete_forward(),
                    37 => self.edit_move_caret(-1, shift),
                    39 => self.edit_move_caret(1, shift),
                    36 => self.edit_set_caret(0, shift),
                    35 => self.edit_set_caret(usize::MAX, shift),
                    13 => {
                        self.commit_edit();
                    }
                    27 => {
                        // Escape discards the edit rather than committing it, which is
                        // the only difference between the two exit paths and therefore
                        // the whole reason both exist.
                        self.edit_buffer = None;
                        self.edit_cursor = 0;
                        self.edit_anchor = None;
                        self.cursor_blink.stop();
                        self.base.request_redraw();
                    }
                    _ => {
                        if let Some(ch) = char::from_u32(*key) {
                            if ch.is_ascii_graphic() || ch == ' ' {
                                self.edit_insert(&ch.to_string());
                            }
                        }
                    }
                }
            }
            // Keyboard navigation of the list: arrows move the selection, Page keys move a
            // windowful, Home/End jump, Escape closes, and a printable key performs
            // first-letter selection.
            Event::KeyPress { key, .. } if !self.editable && !self.visible_fonts.is_empty() => {
                let page = self.max_visible_items.max(1);
                let count = self.visible_fonts.len() as i32;
                match *key {
                    38 => {
                        let next = (self.current_index.max(0) - 1).max(0);
                        self.set_current_index(next);
                    }
                    40 => {
                        let next = (self.current_index + 1).min(count - 1);
                        self.set_current_index(next);
                    }
                    33 => {
                        let next = (self.current_index - page).max(0);
                        self.set_current_index(next);
                    }
                    34 => {
                        let next = (self.current_index + page).min(count - 1);
                        self.set_current_index(next);
                    }
                    36 => self.set_current_index(0),
                    35 => self.set_current_index(count - 1),
                    27 => self.hide_popup(),
                    32..=126 => {
                        if let Some(ch) = char::from_u32(*key) {
                            // A prefix older than the shared timeout no longer describes what
                            // the user is typing, so it is dropped before this key extends it
                            // (I-P06). Clearing queues no frame by itself.
                            let now = crate::compat::Instant::now();
                            let expired = self.type_ahead_started.is_none_or(|started| {
                                now.duration_since(started).as_millis()
                                    > u128::from(
                                        crate::widget::input_widgets::text_match::TYPE_AHEAD_TIMEOUT_MS,
                                    )
                            });
                            if expired {
                                self.type_ahead.clear();
                            }
                            self.type_ahead_started = Some(now);
                            self.type_ahead.push(ch);
                            // Folded once per key and compared byte-wise per row (I-P05).
                            let prefix =
                                crate::widget::input_widgets::text_match::fold(&self.type_ahead);
                            let start = self.current_index.max(-1) as usize + 1;
                            if let Some((offset, _)) =
                                self.visible_fonts.iter().enumerate().skip(start).find(
                                    |(_, name)| {
                                        crate::widget::input_widgets::text_match::starts_with(
                                            name, &prefix,
                                        )
                                    },
                                )
                            {
                                if let Some(original) = self.original_index_at(offset) {
                                    self.set_current_index(original as i32);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::MousePress { pos, button, .. } if button == &1 => {
                self.press_at(*pos);
            }
            // Touch has no translated press here: contact arms the latch and the completed
            // tap performs the same open/commit/dismiss transition as the pointer.
            #[cfg(feature = "touch")]
            Event::TouchBegin { pos, .. } => {
                let inside =
                    self.geometry().contains_point(*pos) || self.popup_row_at(*pos).is_some();
                self.base.set_pressed(inside);
            }
            #[cfg(feature = "touch")]
            event if crate::event::translator::is_touch_cancel(event) => {
                self.base.set_pressed(false);
            }
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                self.press_at(*pos);
                self.base.set_pressed(false);
            }
            Event::MouseRelease { button, .. }
                if button == &1
                    // The press path already opened the popup (or committed a row), so the
                    // release only clears the latch. Cycling here as well made one click run two
                    // selection paths (I-120).
                    && self.base.is_pressed() =>
            {
                self.base.set_pressed(false);
            }
            Event::Wheel { delta, modifiers } => {
                if self.expanded && !self.visible_fonts.is_empty() {
                    let scroll = crate::event::WheelScroll::from_event(*delta, *modifiers, 1);
                    if scroll.lines_y != 0 {
                        let offset = (self.scroll_offset as i32 + scroll.lines_y).max(0) as usize;
                        self.set_scroll_offset(offset);
                    }
                } else {
                    self.base.handle_event(event);
                }
            }
            Event::MouseRelease { button: 1, .. } => {
                self.base.set_pressed(false);
            }
            // Losing focus abandons a held press, so the latch cannot survive a window switch.
            // It also **discards** a half-typed family: the buffer used to survive a blur, so the
            // field kept text that neither Enter had committed nor Escape had cancelled (I-17).
            Event::FocusLost => {
                self.base.set_pressed(false);
                self.cursor_blink.stop();
                if self.edit_buffer.is_some() {
                    self.edit_buffer = None;
                    self.edit_cursor = 0;
                    self.edit_anchor = None;
                    self.base.request_redraw();
                }
            }
            _ => { /* Other events are not relevant */ }
        }
        // Record the primitive facts after the gesture.
        self.base.handle_event(event);
    }
}

impl Draw for FontComboBox {
    fn draw(&mut self, ctx: &mut RenderContext) {
        // ── The field actually painted ──
        //
        // `geometry()` is the area the control was *given*; the **field** of a font combo box is
        // a text field, and a field is a fixed-height band. The expanded popup below it is a
        // menu rather than part of the field, so it still hangs from the control's own bottom
        // edge, which is a separate derivation and is left as one.
        let g = self.field_band();
        if g.width == 0 || g.height == 0 {
            return;
        }

        // Chrome colours resolve the explicit style first, then the theme's resolved style for
        // this control, and only then fall back to a literal. Every colour below used to be a
        // literal, so a light/dark switch left the field, its arrow, its text and its popup
        // unchanged — the rendering census reported the control as theme-blind.
        //
        // The theme read is a separate manager lock, taken and released inside
        // `resolved_theme_style`, so it is not held across the draw — the global manager's mutex
        // is not re-entrant.
        let style = self.base.style();
        // `font_combo_box` classifies as `WidgetRole::Input` (the role table lists it as
        // `fontcombobox`), whose resolved background is the field interior — lighter on a light
        // theme, darker on a dark one — and whose ink is the theme's foreground.
        let theme = crate::style::resolved_theme_style_shared("font_combo_box");
        // Read from the shared palette snapshot and copied out as values, so no manager guard is
        // held while anything else touches the theme.
        let (window_fill, foreground, primary, secondary) = {
            match crate::style::palette_shared() {
                Some(colors) => {
                    (colors.background, colors.foreground, colors.primary, colors.secondary)
                }
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
        // The field: a caller's own colour wins, then the theme's resolved background. The window
        // fill is filtered out because a control can carry it after a theme application, and
        // painting it would make the field indistinguishable from the window. A caller's own
        // colour still wins.
        let field = style
            .background_color
            .filter(|resolved| *resolved != window_fill)
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .filter(|resolved| *resolved != window_fill)
            .unwrap_or_else(|| window_fill.blend(&ink, 0.10));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| field.blend(&secondary, 0.45));
        // The drop-down arrow is an affordance, de-emphasised from the field's own ink so it is
        // legible on either appearance rather than being a fixed grey.
        let arrow_color = ink.blend(&field, 0.35);

        ctx.fill_rect(g, field);
        ctx.draw_rect(g, border);
        // The indicator's box and the value's box come from **one** derivation, so the family
        // name yields to the indicator instead of being written at an unconstrained offset.
        let value_font = Font::default_ui();
        let field_line = ctx.text_line(g, &value_font);
        let geometry = self.field_geometry(field_line.height);
        // Draw drop-down arrow indicator inside its own derived box, vertically centred on the
        // value's own line box rather than on the field's raw midpoint.
        let arrow_indicator_line = ctx.text_line(geometry.indicator_box, &value_font);
        let arrow_center_y = arrow_indicator_line.y + arrow_indicator_line.height as i32 / 2;
        let arrow_center_x = geometry.indicator_box.x + geometry.indicator_box.width as i32 / 2;
        ctx.draw_line(
            Point::new(arrow_center_x - 3, arrow_center_y - 2),
            Point::new(arrow_center_x, arrow_center_y + 2),
            arrow_color,
        );
        ctx.draw_line(
            Point::new(arrow_center_x, arrow_center_y + 2),
            Point::new(arrow_center_x + 3, arrow_center_y - 2),
            arrow_color,
        );
        // Borrowed, not copied: this is one draw per frame and the family name already lives
        // in the control's own `Font`. An edit under way shows its own buffer instead (I-16).
        let edit_text = self.edit_buffer.as_deref();
        let font_name = match edit_text {
            Some(buffer) => buffer,
            None => self.current_font().family(),
        };
        // The field's own line box. A glyph origin is the box's top-left edge, so the old
        // `g.y + g.height / 2 + 5` put that edge on the field's middle line and drew the
        // selected family half a line low.
        let value_line = ctx.text_line(geometry.value_box, &value_font);
        // Fitted, so a long family name is elided rather than drawn under the indicator — the box
        // it is given is the one the indicator left.
        ctx.draw_text_fitted_cow(
            Rect::new(
                geometry.value_box.x,
                value_line.y,
                geometry.value_box.width,
                value_line.height,
            ),
            font_name,
            &value_font,
            ink,
            self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left),
        );
        // The edit's range and caret, drawn on the value's own line box so the marker rides
        // with the glyphs (I-16). Before this an editable box could only append and pop from the
        // tail, so the field never said where in the family name typing would land.
        if edit_text.is_some() {
            let align = self.alignment.to_horizontal().unwrap_or(HorizontalAlignment::Left);
            let value_advance = ctx.measure_text(font_name, &value_font).width as i32;
            let origin = crate::render::fitted_origin(
                Rect::new(
                    geometry.value_box.x,
                    value_line.y,
                    geometry.value_box.width,
                    value_line.height,
                ),
                value_advance,
                align,
            );
            if let Some((start, end)) = self.edit_selection_range() {
                let start = start.min(font_name.len());
                let end = end.min(font_name.len());
                let start_x =
                    origin.x + ctx.measure_text(&font_name[..start], &value_font).width as i32;
                let end_x =
                    origin.x + ctx.measure_text(&font_name[..end], &value_font).width as i32;
                ctx.fill_rect(
                    Rect::new(
                        start_x.min(end_x),
                        value_line.y,
                        (end_x - start_x).unsigned_abs(),
                        value_line.height.max(1),
                    ),
                    ink.with_alpha(70),
                );
            }
            let caret = self.edit_cursor.min(font_name.len());
            let caret_x =
                origin.x + ctx.measure_text(&font_name[..caret], &value_font).width as i32;
            let caret_x = caret_x
                .min(geometry.value_box.x + geometry.value_box.width as i32 - 1)
                .max(geometry.value_box.x);
            if self.cursor_blink.is_visible() {
                ctx.draw_line(
                    Point::new(caret_x, value_line.y),
                    Point::new(caret_x, value_line.y + value_line.height as i32),
                    ink,
                );
            }
        }
        // ── Keyboard focus ring ──
        //
        // Around the field band — the part the keyboard operates on — rather than around the
        // control's laid-out rectangle, which can be taller than the field.
        if self.base.draws_focus_ring() {
            let ring = FocusRing::for_control(g, 0);
            if ring.is_drawable() {
                ctx.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    focus_ring_color(field.contrast_color()),
                    FOCUS_RING_WIDTH,
                );
            }
        }
        // Draw popup list when expanded
        if self.expanded && !self.visible_fonts.is_empty() {
            // One row height for the paint and the hit test, derived from the effective font
            // rather than the two fixed numerals that used to disagree (I-15).
            let popup_item_height = self.popup_row_height().max(1);
            let page = self.page_size(self.visible_fonts.len());
            let offset = self
                .scroll_offset
                .min(self.visible_fonts.len().saturating_sub(page.min(self.visible_fonts.len())));
            let visible =
                &self.visible_fonts[offset..(offset + page).min(self.visible_fonts.len())];
            let popup_height = (visible.len() as i32) * popup_item_height;
            let popup_rect = Rect::new(g.x, g.y + g.height as i32, g.width, popup_height as u32);
            // The popup is a raised step out of the field, so it reads as a separate surface on
            // either appearance.
            let popup_fill = field.blend(&ink, 0.06);
            ctx.fill_rect(popup_rect, popup_fill);
            ctx.draw_rect(popup_rect, border);
            // The highlighted item is the control's selection, so it carries the theme's primary
            // rather than a fixed blue.
            let highlight = primary.blend(&field, 0.15);
            let display_font_size = self.popup_row_font_size();
            // One preview font per visible row, cached until the list or the size changes: each
            // row used to build `Font::simple(name.clone(), ..)` every frame (I-P08).
            self.with_popup_fonts(display_font_size, |row_fonts| {
                for (row, name) in visible.iter().enumerate() {
                    let item_y = popup_rect.y + (row as i32) * popup_item_height;
                    let item_rect =
                        Rect::new(popup_rect.x, item_y, popup_rect.width, popup_item_height as u32);
                    // Each row is drawn in its own family, which is the point of a font list:
                    // previewing every family in one default face told the user nothing.
                    let Some(display_font) = row_fonts.get(offset + row) else { continue };
                    let item_line = ctx.text_line(item_rect, display_font);
                    let name_x = g.x + 4;
                    let name_width =
                        ((item_rect.x + item_rect.width as i32 - 4) - name_x).max(0) as u32;
                    // The row's index in the unfiltered list comes from the parallel index table
                    // rather than a linear search per painted row (I-P08).
                    let original = self.original_index_at(offset + row);
                    if original == Some(self.current_index as usize) {
                        ctx.fill_rect(item_rect, highlight);
                        ctx.draw_text_fitted_cow(
                            Rect::new(
                                name_x,
                                item_line.y,
                                name_width.max(1),
                                item_line.height.max(1),
                            ),
                            name,
                            display_font,
                            highlight.contrast_color(),
                            HorizontalAlignment::Left,
                        );
                    } else {
                        ctx.draw_text_fitted_cow(
                            Rect::new(
                                name_x,
                                item_line.y,
                                name_width.max(1),
                                item_line.height.max(1),
                            ),
                            name,
                            display_font,
                            ink,
                            HorizontalAlignment::Left,
                        );
                    }
                    if let Some(sample) = &self.sample_text {
                        // The preview column uses the row's own family too, so a family that
                        // shapes text differently from the default face is visible as such.
                        let sample_box = Rect::new(
                            name_x + (name_width as i32 / 2),
                            item_line.y,
                            (name_width as i32 / 2).max(1) as u32,
                            item_line.height.max(1),
                        );
                        ctx.draw_text_fitted_cow(
                            sample_box,
                            sample,
                            display_font,
                            ink.blend(&field, 0.35),
                            HorizontalAlignment::Right,
                        );
                    }
                }
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn fontcombobox_creation_defaults() {
        let fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(fcb.fonts().is_empty());
        assert_eq!(fcb.count(), 0);
        assert_eq!(fcb.selected_index().map(|index| index as i32).unwrap_or(-1), -1);
        assert!(fcb.current_text().is_empty());
        assert!(!fcb.is_editable());
        assert_eq!(fcb.max_visible_items(), 10);
    }

    #[test]
    fn fontcombobox_add_font() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        assert_eq!(fcb.count(), 2);
        assert_eq!(fcb.fonts()[0], "Arial");
        assert_eq!(fcb.fonts()[1], "Helvetica");
    }

    #[test]
    fn fontcombobox_load_system_fonts_fills_a_sorted_list() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        let count = fcb.load_system_fonts();
        assert_eq!(fcb.count(), i32::try_from(count).expect("font count fits i32"));
        assert_eq!(fcb.fonts().len(), count);
        assert!(fcb
            .fonts()
            .windows(2)
            .all(|pair| pair[0].to_lowercase() <= pair[1].to_lowercase()));
    }

    #[test]
    fn fontcombobox_remove_font() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        fcb.remove_font(0);
        assert_eq!(fcb.count(), 1);
        assert_eq!(fcb.fonts()[0], "Helvetica");
    }

    #[test]
    fn fontcombobox_clear() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        fcb.set_current_index(0);
        fcb.clear();
        assert_eq!(fcb.count(), 0);
        assert_eq!(fcb.selected_index().map(|index| index as i32).unwrap_or(-1), -1);
    }

    #[test]
    fn fontcombobox_set_current_index() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.add_font("Arial".to_string());
        fcb.add_font("Helvetica".to_string());
        fcb.set_current_index(0);
        assert_eq!(fcb.selected_index().map(|index| index as i32).unwrap_or(-1), 0);
        assert_eq!(fcb.current_text(), "Arial");
    }

    #[test]
    fn fontcombobox_set_current_font() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        let font = Font::new("Arial", 12.0, false, false);
        fcb.set_current_font(font.clone());
        assert_eq!(fcb.current_font().family(), "Arial");
    }

    #[test]
    fn fontcombobox_editable() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(!fcb.is_editable());
        fcb.set_editable(true);
        assert!(fcb.is_editable());
        fcb.set_editable(false);
        assert!(!fcb.is_editable());
    }

    #[test]
    fn fontcombobox_max_visible_items() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert_eq!(fcb.max_visible_items(), 10);
        fcb.set_max_visible_items(5);
        assert_eq!(fcb.max_visible_items(), 5);
        fcb.set_max_visible_items(0); // floors at 1
        assert_eq!(fcb.max_visible_items(), 1);
    }

    #[test]
    fn fontcombobox_show_hide_popup() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.show_popup();
        fcb.hide_popup();
        // Should not panic
    }

    #[test]
    fn fontcombobox_geometry_delegation() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        fcb.set_geometry(Rect::new(10, 10, 300, 30));
        assert_eq!(fcb.geometry(), Rect::new(10, 10, 300, 30));
    }

    #[test]
    fn fontcombobox_visibility() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(fcb.is_visible());
        fcb.hide();
        assert!(!fcb.is_visible());
        fcb.show();
        assert!(fcb.is_visible());
    }

    #[test]
    fn fontcombobox_enabled() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        assert!(fcb.is_enabled());
        fcb.set_enabled(false);
        assert!(!fcb.is_enabled());
        fcb.set_enabled(true);
        assert!(fcb.is_enabled());
    }

    #[test]
    fn fontcombobox_id_kind() {
        let fcb_a = FontComboBox::new(Rect::new(0, 0, 100, 24));
        let fcb_b = FontComboBox::new(Rect::new(0, 0, 100, 24));
        assert_ne!(fcb_a.id(), fcb_b.id());
        assert_eq!(fcb_a.kind(), WidgetKind::FontComboBox);
        assert_eq!(fcb_b.kind(), WidgetKind::FontComboBox);
    }

    #[test]
    fn fontcombobox_signal_accessors() {
        let fcb = FontComboBox::new(Rect::new(0, 0, 100, 24));
        let _ = &fcb.current_font_changed;
        let _ = &fcb.current_index_changed;
        let _ = &fcb.activated;
        let _ = &fcb.text_edited;
        let _ = &fcb.popup_shown;
        let _ = &fcb.popup_hidden;
    }

    #[test]
    fn font_combo_box_draw_produces_output() {
        let mut fcb = FontComboBox::new(Rect::new(0, 0, 200, 24));
        let svg = crate::widget::svg::render_to_svg(&mut fcb);
        assert!(svg.starts_with("<svg"));
    }
    /// Only a completed press on the control cycles the font.
    ///
    /// `handle_event` cycled on `Event::MouseRelease` with the position discarded (`pos: _`) and
    /// no press-arm, so **any** left release reached the arm: a release the host routed from a drag
    /// that began on another control, or one with no preceding press at all. This is the same
    /// defect class as `Switch`; `Button`, `CheckBox` and `Switch` all gate on a press-provided
    /// flag.
    #[test]
    fn font_combo_box_cycles_only_on_a_completed_press() {
        let inside = Point::new(50, 15);
        let outside = Point::new(9000, 9000);
        let build = || {
            let mut c = FontComboBox::new(Rect::new(0, 0, 200, 30));
            c.add_font("Arial".to_string());
            c.add_font("Helvetica".to_string());
            c.add_font("Menlo".to_string());
            c
        };
        let run = |events: &[Event]| {
            let mut c = build();
            let before = c.selected_index().map(|index| index as i32).unwrap_or(-1);
            for event in events {
                c.handle_event(event);
            }
            (before, c.selected_index().map(|index| index as i32).unwrap_or(-1))
        };

        // A release with no press must not cycle, wherever it lands.
        assert_eq!(run(&[Event::MouseRelease { pos: inside, button: 1 }]), (-1, -1));
        assert_eq!(run(&[Event::MouseRelease { pos: outside, button: 1 }]), (-1, -1));

        // A press outside does not arm the latch.
        assert_eq!(
            run(&[
                Event::MousePress { pos: outside, button: 1, modifiers: 0 },
                Event::MouseRelease { pos: inside, button: 1 },
            ]),
            (-1, -1),
            "a drag that began outside must not commit"
        );

        // Pressing inside and releasing outside cancels.
        assert_eq!(
            run(&[
                Event::MousePress { pos: inside, button: 1, modifiers: 0 },
                Event::MouseRelease { pos: outside, button: 1 },
            ]),
            (-1, -1)
        );

        // A completed press opens the popup; the release does not also advance the
        // selection, which was the second trigger path this change removed (I-120).
        assert_eq!(
            run(&[
                Event::MousePress { pos: inside, button: 1, modifiers: 0 },
                Event::MouseRelease { pos: inside, button: 1 },
            ]),
            (-1, -1)
        );

        // Focus loss abandons a held press.
        assert_eq!(
            run(&[
                Event::MousePress { pos: inside, button: 1, modifiers: 0 },
                Event::FocusLost,
                Event::MouseRelease { pos: inside, button: 1 },
            ]),
            (-1, -1)
        );
    }

    /// A right-button release is ignored, armed or not.
    #[test]
    fn font_combo_box_ignores_non_primary_releases() {
        let inside = Point::new(50, 15);
        let mut c = FontComboBox::new(Rect::new(0, 0, 200, 30));
        c.add_font("Arial".to_string());
        c.add_font("Helvetica".to_string());
        c.handle_event(&Event::MouseRelease { pos: inside, button: 3 });
        assert_eq!(c.selected_index().map(|index| index as i32).unwrap_or(-1), -1);
    }

    /// The family name's box ends where the indicator's box begins, at every control width.
    ///
    /// # What this pins
    ///
    /// BLUE22 §B.6 rule 4: a sub-part's box is derived from its sibling, so the name *yields*
    /// to the indicator. The name used to be written at `g.x + 4` with no upper bound while
    /// the indicator sat at `g.x + g.width - 14`, so a long family such as
    /// `Noto Sans CJK JP` in a narrow format bar was drawn underneath the arrow.
    #[test]
    fn the_family_name_box_ends_where_the_indicator_begins() {
        let trailing =
            FONT_FIELD_PADDING as u32 + dimensions::BUTTON_ICON_SIZE + INDICATOR_LEADING_GAP;
        for width in [0u32, 20, 64, 240, 400] {
            let c = FontComboBox::new(Rect::new(0, 0, width, 120));
            let geometry = c.field_geometry(14);
            let band = c.field_band();
            assert_eq!(
                geometry.value_box.x
                    + geometry.value_box.width as i32
                    + INDICATOR_LEADING_GAP as i32,
                geometry.indicator_box.x,
                "the name must stop one gap short of the indicator at width {width}"
            );
            assert!(
                geometry.indicator_box.x + geometry.indicator_box.width as i32
                    <= band.x + band.width as i32,
                "the indicator must stay inside the band at width {width}"
            );
            if width < trailing {
                assert_eq!(
                    geometry.value_box.width, 0,
                    "a band too narrow for its own chrome holds no name at width {width}"
                );
            }
        }
    }

    /// The reported height is the band that is painted.
    #[test]
    fn the_reported_height_is_the_band_that_is_painted() {
        let c = FontComboBox::new(Rect::new(0, 0, 240, 120));
        let band = c.field_band();
        assert_eq!(band.height, dimensions::TEXT_FIELD_MIN_HEIGHT);
        assert_eq!(c.size_hint().height, band.height);
        assert_eq!(band.y, (120 - dimensions::TEXT_FIELD_MIN_HEIGHT as i32) / 2);
    }

    #[test]
    fn setting_the_visible_item_count_repaints() {
        let mut combo = FontComboBox::new(Rect::new(0, 0, 200, 200));
        let seen = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        combo.base.redraw_requested.connect({
            let seen = std::sync::Arc::clone(&seen);
            move || {
                seen.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            }
        });

        combo.set_max_visible_items(5);
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the list's height is derived from this"
        );
        combo.set_max_visible_items(5);
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the same count is not a change"
        );
        combo.set_max_visible_items(0);
        assert_eq!(
            seen.load(std::sync::atomic::Ordering::SeqCst),
            2,
            "zero floors to one, which is a change"
        );
    }

    /// Style and writing-system filters narrow the visible font list.
    #[test]
    fn font_filters_narrow_the_list() {
        let mut combo = FontComboBox::new(Rect::new(0, 0, 200, 28));
        for family in ["Inter", "JetBrains Mono", "Courier New", "CJK Sans"] {
            combo.add_font(family.to_string());
        }
        combo.set_font_filters(FontFilters::MONOSPACED);
        assert_eq!(combo.font_filters(), FontFilters::MONOSPACED);
        assert_eq!(combo.visible_fonts().len(), 2);
        assert!(combo.visible_fonts().iter().all(|name| FontFilters::MONOSPACED.matches(name)));

        combo.set_font_filters(FontFilters::NONE);
        combo.set_writing_system_filter(Some("CJK".to_string()));
        assert_eq!(combo.writing_system_filter(), Some("CJK"));
        assert_eq!(combo.visible_fonts(), ["CJK Sans".to_string()].as_slice());

        combo.set_sample_text(Some("Aa 123".to_string()));
        assert_eq!(combo.sample_text(), Some("Aa 123"));
        combo.set_sample_text(None);
        assert_eq!(combo.sample_text(), None);
    }

    /// The filter tokens and the sample text round-trip through the property route.
    #[test]
    fn the_filter_properties_round_trip() {
        let mut combo = FontComboBox::new(Rect::new(0, 0, 200, 28));
        for family in ["Inter", "JetBrains Mono"] {
            combo.add_font(family.to_string());
        }
        assert_eq!(
            combo.get("font_filters").unwrap(),
            CapabilityValue::String(String::new()),
            "no filters is the empty token list"
        );
        assert_eq!(combo.get("writing_system_filter").unwrap(), CapabilityValue::Null);
        assert_eq!(combo.get("sample_text").unwrap(), CapabilityValue::Null);

        combo
            .set("font_filters", CapabilityValue::String("monospaced,scalable".to_string()))
            .unwrap();
        assert!(combo.font_filters().contains(FontFilters::MONOSPACED));
        assert!(combo.font_filters().contains(FontFilters::SCALABLE));
        assert_eq!(
            combo.get("font_filters").unwrap(),
            CapabilityValue::String("monospaced,scalable".to_string())
        );
        assert_eq!(
            combo.set("font_filters", CapabilityValue::String("heavy".to_string())),
            Err(CapabilityAccessError::OutOfRange)
        );

        combo.set("writing_system_filter", CapabilityValue::String("CJK".to_string())).unwrap();
        assert_eq!(combo.writing_system_filter(), Some("CJK"));
        combo.set("writing_system_filter", CapabilityValue::Null).unwrap();
        assert_eq!(combo.writing_system_filter(), None);

        combo.set("sample_text", CapabilityValue::String("Aa".to_string())).unwrap();
        assert_eq!(combo.get("sample_text").unwrap(), CapabilityValue::String("Aa".to_string()));
    }

    /// A capped popup scrolls, and a press outside dismisses it.
    #[test]
    fn the_popup_scrolls_and_dismisses_on_an_outside_press() {
        let mut combo = FontComboBox::new(Rect::new(0, 0, 200, 28));
        for index in 0..10 {
            combo.add_font(format!("Family {index}"));
        }
        combo.set_max_visible_items(3);
        combo.show_popup();
        combo.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(combo.scroll_offset(), 1);
        assert_eq!(combo.page_size(10), 3);

        combo.handle_event(&Event::MousePress {
            pos: Point::new(500, 500),
            button: 1,
            modifiers: 0,
        });
        assert!(!combo.expanded, "an outside press dismisses the popup");
    }

    #[test]
    fn the_scalable_filter_has_a_consumption_point() {
        assert!(FontFilters::SCALABLE.matches("Noto Sans"));
        assert!(!FontFilters::SCALABLE.matches("Console Raster 8"));
        assert!(FontFilters::NONE.matches("Console Raster 8"));
    }

    #[test]
    fn recently_chosen_families_lead_the_list_when_mru_is_on() {
        let mut combo = FontComboBox::new(Rect::new(0, 0, 200, 28));
        for name in ["Alpha", "Bravo", "Charlie"] {
            combo.add_font(name.to_string());
        }
        assert!(!combo.mru_enabled());
        combo.set_current_index(2);
        assert!(combo.recent_fonts().is_empty(), "off by default records nothing");
        assert_eq!(combo.visible_fonts()[0], "Alpha");

        combo.set_mru_enabled(true);
        combo.set_current_index(1);
        combo.set_current_index(2);
        assert_eq!(combo.recent_fonts()[0], "Charlie");
        assert_eq!(combo.visible_fonts()[0], "Charlie");
    }

    #[test]
    fn one_click_on_the_field_only_opens_the_popup() {
        let mut combo = FontComboBox::new(Rect::new(0, 0, 200, 28));
        for name in ["Alpha", "Bravo", "Charlie"] {
            combo.add_font(name.to_string());
        }
        let center = Point::new(20, 14);
        combo.handle_event(&Event::MousePress { pos: center, button: 1, modifiers: 0 });
        combo.handle_event(&Event::MouseRelease { pos: center, button: 1 });
        assert!(combo.expanded, "the press opens the popup");
        assert_eq!(combo.selected_index(), None, "the release must not also cycle the font");
    }
}
