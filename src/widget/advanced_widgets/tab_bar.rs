// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Standalone TabBar widget — decoupled from TabWidget, draws a row/column of tabs.
use crate::compat::Cow;
use crate::core::{Color, Font, HorizontalAlignment, ObjectId, Point, Rect, Size};
use crate::event::{Event, EventHandler, WheelScroll};
use crate::render::RenderContext;
use crate::signal::{GenericSignal, Signal1};

use crate::widget::capability::coercion::{
    expect_bool, expect_text_direction, expect_u32_saturating, expect_usize, text_direction_to_str,
};
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::companions::TooltipManager;
use crate::widget::container_widgets::tabwidget::{TabPosition, TabShape};
use crate::widget::metrics::{
    expand_to_hit_area, focus_ring_color, role_font, FocusRing, FOCUS_RING_WIDTH,
};
use crate::widget::text_utils::elide_to_width;
use crate::widget::{
    BaseWidget, Draw, IconItemError, IconName, IconSource, Image, Widget, WidgetKind,
};
use crate::{impl_widget_property_hooks, property_names_of};

const TAB_HEIGHT: i32 = 24;
const TAB_MIN_WIDTH: u32 = 40;
const TAB_MAX_WIDTH: u32 = 200;
const TAB_SPACING: i32 = 2;
const CLOSE_SIZE: i32 = 12;
const CLOSE_PADDING: i32 = 5;

/// Font size every tab label is drawn at, in points (see [`TabBar::draw_tab`]).
const TAB_FONT_SIZE: f32 = 14.0;

/// Space reserved either side of a tab's label inside its own box.
///
/// The label starts [`TAB_TEXT_INSET`] from the tab's left edge and the same distance is
/// kept free on the right; the width reservation has to cover both, or the last glyph
/// would sit against the border.
const TAB_TEXT_INSET: u32 = 6;
const TAB_TEXT_PADDING: u32 = TAB_TEXT_INSET * 2;
const TAB_ICON_SIZE: u32 = 16;
const TAB_ICON_GAP: u32 = 4;

/// The advance the renderer will give a label before the first `draw` has measured it.
///
/// Delegates to the crate's one width estimate ([`crate::widget::metrics::estimate_text_width`])
/// rather than carrying a private one. The private copy counted `chars() * font_size`, which
/// disagrees with the shared estimate on every input that estimate cares about — a wide cluster,
/// a cluster that is not one `char` (combining marks, ZWJ emoji), or a font with `letter_spacing`
/// — so a tab's reserved width and its painted width could differ before the first measurement.
/// This is the same "one metric, not two" rule the shared estimate exists for (BLUE24 U-14).
fn estimate_text_width(text: &str, font: &Font) -> u32 {
    crate::widget::metrics::estimate_text_width(text, font, 1.0)
}

/// The stable hover id for one tab.
///
/// The tooltip pipeline addresses a hover source by id; a tab is not a widget, so the id is
/// derived from the strip's own and the tab's index. The mix is the toolbar's, so the two controls
/// cannot collide on an accidental linear id.
fn hover_id(owner: ObjectId, index: usize) -> ObjectId {
    owner.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (index as u64 + 1)
}

/// The cache key for one tab's elided title: the text, the label band, the font and the mode.
fn elide_key(text: &str, budget: u32, font: &Font, mode: TabElideMode) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    let mut mix = |value: u64| {
        hash ^= value;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for byte in text.as_bytes() {
        mix(u64::from(*byte));
    }
    mix(0xFF);
    mix(u64::from(budget));
    mix(u64::from(font.size().to_bits()));
    mix(u64::from(font.weight()));
    for byte in font.family().as_bytes() {
        mix(u64::from(*byte));
    }
    mix(match mode {
        TabElideMode::None => 0,
        TabElideMode::Right => 1,
        TabElideMode::Left => 2,
        TabElideMode::Middle => 3,
    });
    hash
}

/// Shortens `text` so its measured width fits `max_width`, keeping the **tail**.
///
/// The width-budget counterpart of [`crate::widget::text_utils::elide_start`], which is the
/// character-count form. Same contract as `elide_to_width`: fitting text is returned borrowed, the
/// ellipsis is part of the budget, and a budget too small for the ellipsis itself yields an empty
/// slice rather than a glyph that overflows. The largest fitting suffix is found by binary search
/// over character boundaries, so a multi-byte tail is never cut mid-character.
fn elide_start_to_width<'a>(
    text: &'a str,
    max_width: u32,
    measure: &mut dyn FnMut(&str) -> u32,
) -> Cow<'a, str> {
    const ELLIPSIS: char = '\u{2026}';
    if measure(text) <= max_width {
        return Cow::Borrowed(text);
    }
    let ellipsis_width = measure(&ELLIPSIS.to_string());
    if max_width == 0 || ellipsis_width > max_width {
        return Cow::Borrowed("");
    }
    // The ellipsis has to be measured as part of every candidate, so the suffix budget is what is
    // left after it.
    let char_count = text.chars().count();
    let mut low = 0usize;
    let mut high = char_count;
    while low < high {
        let mid = (low + high).div_ceil(2);
        let start = byte_index_from_end(text, mid);
        let mut candidate = String::with_capacity(3 + text.len() - start);
        candidate.push(ELLIPSIS);
        candidate.push_str(&text[start..]);
        if measure(&candidate) <= max_width {
            low = mid;
        } else {
            high = mid - 1;
        }
    }
    if low == char_count {
        return Cow::Borrowed(text);
    }
    let start = byte_index_from_end(text, low);
    let mut out = String::with_capacity(3 + text.len() - start);
    out.push(ELLIPSIS);
    out.push_str(&text[start..]);
    Cow::Owned(out)
}

/// The byte index of the first character of the last `keep` characters of `text`.
fn byte_index_from_end(text: &str, keep: usize) -> usize {
    let char_count = text.chars().count();
    if keep >= char_count {
        return 0;
    }
    let skip = char_count - keep;
    text.char_indices().nth(skip).map(|(index, _)| index).unwrap_or(text.len())
}

/// The elided form of one tab's title, cached between frames.
#[derive(Clone, Debug)]
enum ElidedTitle {
    /// The full title fits its label band and is painted from the tab itself.
    Fits,
    /// The title was shortened and this is the string to paint.
    Elided(String),
}

/// A single tab in a `TabBar`.
pub struct TabBarTab {
    title: String,
    icon: Option<Image>,
    icon_source: Option<IconSource>,
    tooltip: String,
    enabled: bool,
}

impl TabBarTab {
    /// Creates a new tab with the given title.
    pub fn new(title: String) -> Self {
        Self { title, icon: None, icon_source: None, tooltip: String::new(), enabled: true }
    }

    /// Returns the tab title.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Sets the tab title.
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    /// Returns the tab icon, if any.
    pub fn icon(&self) -> Option<&Image> {
        self.icon.as_ref()
    }

    /// Sets the tab icon.
    ///
    /// A non-RGBA8 image is normalised once here, so the draw path can borrow its bytes instead of
    /// transcoding every frame — and so an image this crate cannot draw is not left in the slot.
    pub fn set_icon(&mut self, icon: Option<Image>) {
        self.icon = icon.map(crate::widget::icon_source::normalize_rgba8_image);
        self.icon_source = None;
    }

    /// Returns the typed icon source, if one is assigned.
    pub fn icon_source(&self) -> Option<IconSource> {
        self.icon_source
    }

    /// Replaces the typed icon source and clears the legacy image slot.
    ///
    /// An explicit `None` clears **both** forms: leaving the image behind would make the tab keep
    /// drawing an icon the caller just cleared.
    pub fn set_icon_source(&mut self, source: Option<IconSource>) {
        self.icon_source = source;
        self.icon = None;
    }

    /// Whether this tab reserves a leading icon slot.
    pub fn has_icon(&self) -> bool {
        self.icon_source.is_some() || self.icon.is_some()
    }

    /// Returns the tab tooltip.
    pub fn tooltip(&self) -> &str {
        &self.tooltip
    }

    /// Sets the tab tooltip.
    pub fn set_tooltip(&mut self, tooltip: String) {
        self.tooltip = tooltip;
    }

    /// Returns whether the tab is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Sets the enabled state of the tab.
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
}

/// The declared shape of one tab row, as a record (BLUE26 B26.09).
///
/// This is the row shape of the `tabs` property, and the control owns both ends of it: the field
/// names are [`TabBarTab`]'s own accessors. `title` is required because [`TabBarTab::new`]
/// requires it; the rest are optional because a document that omits them describes the ordinary
/// tab. The legacy `icon` image slot has no document form (a decoded image is not a property
/// value); a typed `icon_source` is the persistable spelling.
pub const TAB_BAR_TAB_SCHEMA: crate::widget::capability::RecordSchema =
    crate::widget::capability::RecordSchema {
        fields: &[
            crate::widget::capability::RecordField::required(
                "title",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "tooltip",
                crate::widget::capability::PropertyValueKind::String,
            ),
            crate::widget::capability::RecordField::optional(
                "enabled",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            crate::widget::capability::RecordField::optional(
                "icon_source",
                crate::widget::capability::PropertyValueKind::IconSpec,
            ),
        ],
        // A tab has no separate identity field. Its title is what a caller addresses it by in a
        // document (and two tabs may legitimately share a title), so the row is keyed by `title`
        // with the uniqueness rule off — the shape a positional list with a label would have.
        id_field: "title",
        ids_unique: false,
        ids_required: false,
    };

/// One row of [`TAB_BAR_TAB_SCHEMA`]; refuses a source with no document form.
fn tab_bar_tab_record(
    tab: &TabBarTab,
) -> Result<crate::widget::capability::RecordValue, CapabilityAccessError> {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("title", CapabilityValue::String(tab.title().to_string()))
        .with("enabled", CapabilityValue::Bool(tab.is_enabled()));
    if !tab.tooltip().is_empty() {
        record = record.with("tooltip", CapabilityValue::String(tab.tooltip().to_string()));
    }
    match tab.icon_source() {
        None => {}
        Some(IconSource::Builtin(name)) => {
            record = record.with(
                "icon_source",
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name)),
            );
        }
        // A borrowed static geometry has no document form, so the read refuses rather than
        // reporting the row as icon-less.
        Some(IconSource::StaticGeometry(_)) => {
            return Err(CapabilityAccessError::IconSourceNotPersistable)
        }
    }
    Ok(record)
}

/// Rebuilds one [`TabBarTab`] from a row of [`TAB_BAR_TAB_SCHEMA`].
fn tab_bar_tab_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<TabBarTab, CapabilityAccessError> {
    let title = match record.get("title") {
        Some(CapabilityValue::String(title)) => title.clone(),
        _ => return Err(CapabilityAccessError::TypeMismatch),
    };
    let tooltip = match record.get("tooltip") {
        None | Some(CapabilityValue::Null) => String::new(),
        Some(CapabilityValue::String(tooltip)) => tooltip.clone(),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let enabled = match record.get("enabled") {
        None | Some(CapabilityValue::Null) => true,
        Some(CapabilityValue::Bool(flag)) => *flag,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let icon_source = match record.get("icon_source") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name))) => {
            Some(IconSource::Builtin(*name))
        }
        // A resource descriptor needs the host's resolver, which a property write does not have.
        Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Resource {
            ..
        })) => return Err(CapabilityAccessError::ResourceResolutionRequired),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let mut tab = TabBarTab::new(title);
    tab.set_tooltip(tooltip);
    tab.set_enabled(enabled);
    tab.set_icon_source(icon_source);
    Ok(tab)
}

/// How a tab title that does not fit its tab is handled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabElideMode {
    /// Paint the full title even where it does not fit; the renderer clips it at the tab.
    None,
    /// Elide the title at the tab's trailing edge, the historical rendering.
    #[default]
    Right,
    /// Elide the title at its **leading** edge, keeping the identifying tail.
    ///
    /// The mode a file name wants: a list of `report-2026-10.csv` tabs keeps the extension and the
    /// distinguishing suffix instead of three tabs that all read `report-…`.
    Left,
    /// Elide the **middle**, keeping both ends.
    ///
    /// For a path or URL whose head and tail are each identifying.
    Middle,
}

/// What becomes current after the current tab is removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SelectionBehaviorOnRemove {
    /// Move to the previous tab, or the first when the removed tab was first.
    #[default]
    SelectPrevious,
    /// Move to the next tab, or the last when the removed tab was last.
    SelectNext,
}

/// The strip's visual variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TabBarStyle {
    /// Ordinary tabs with an outlined current tab.
    #[default]
    Standard,
    /// A connected segmented-control strip: the current tab is an accent pill.
    Segmented,
}

/// Standalone tab bar widget that displays a row (or column) of tabs.
pub struct TabBar {
    base: BaseWidget,
    tabs: Vec<TabBarTab>,
    current_index: Option<usize>,
    hovered_index: Option<usize>,
    tab_position: TabPosition,
    tab_shape: TabShape,
    closable: bool,
    movable: bool,
    /// Index of the tab a drag started on, when a move gesture is in progress.
    ///
    /// Tracked so a release can complete a move that began with a press: without
    /// it a drag has no origin, and `tab_moved` has nothing to report.
    dragging_from: Option<usize>,
    tab_min_width: u32,
    tab_max_width: u32,
    /// Per-tab label widths, measured by the renderer on the last `draw`.
    ///
    /// `RenderContext` is the only component that knows the font metrics the backend
    /// will actually use, so the width a tab reserves has to come from there rather
    /// than from a private estimate. Caching it in `draw` (the same shape `GroupBox`
    /// uses for its title) makes hit testing, layout and painting agree *after* the
    /// first frame, and [`estimate_text_width`] only has to carry the first one.
    measured_title_widths: Vec<u32>,
    /// The font-and-titles fold [`Self::measured_title_widths`] was measured for (K-P09).
    ///
    /// `draw` used to clear the widths and re-measure every title on every frame, which is one
    /// shaped-text measurement per tab per paint for a label that had not changed. The fold covers
    /// the font and every title, so any edit to either re-measures and nothing else does.
    measured_title_signature: u64,
    /// The elided title to paint per tab, and the inputs it was elided for (K-P09).
    ///
    /// `None` means the title fits and is painted borrowed. The entries are refreshed only when the
    /// title, the label band, the font or the elide mode changes, so a static strip does no elision
    /// work at all after the first frame.
    elided_titles: Vec<Option<(u64, ElidedTitle)>>,
    /// The writing direction the horizontal strip runs in.
    ///
    /// # Why a tab strip needs this
    ///
    /// A tab strip is a *sequence*: "the first tab is where the strip begins" is a statement about
    /// the line's reading order, not about its geometry. In an Arabic or Hebrew interface the strip
    /// begins at the right, so tab 0 belongs on the right and the overflow grows leftward — otherwise
    /// the visual order of the tabs contradicts the order of the pages they select, and the keyboard
    /// arrows move along the strip the opposite way from the one the reader's eye follows.
    ///
    /// Only the horizontal positions are affected: `West`/`East` strips are *not* mirrored onto each
    /// other, because the side a strip is attached to is a layout decision rather than a reading one.
    /// Defaults to left-to-right, so a strip that never asks behaves exactly as it did.
    direction: crate::core::TextDirection,
    /// The tab index the pointer is dragging over, while a reorder is in flight.
    drag_target: Option<usize>,
    /// Whether the strip paints scroll buttons when the tabs overflow.
    scroll_buttons: bool,
    /// How many tabs the scroll buttons have shifted the run.
    scroll_offset: usize,
    /// How a title that does not fit is handled.
    elide_mode: TabElideMode,
    /// Whether the strip hides itself when it has at most one tab.
    auto_hide: bool,
    /// Whether the tabs stretch to fill the whole strip.
    expanding: bool,
    /// What becomes current when the current tab is removed.
    selection_behavior_on_remove: SelectionBehaviorOnRemove,
    /// The strip's visual variant.
    style_variant: TabBarStyle,
    /// Emitted with the tab under the pointer, or `None` over empty strip space.
    pub tab_bar_clicked: Signal1<Option<usize>>,
    /// Emitted when the current tab index changes.
    pub current_changed: Signal1<usize>,
    /// Emitted when a tab close is requested (closable tabs only).
    pub tab_close_requested: Signal1<usize>,
    /// Emitted when a tab is moved; payload is `(old_index, new_index)`.
    pub tab_moved: Signal1<(usize, usize)>,
    /// Emitted when the strip's empty area is double-clicked, requesting a new tab.
    ///
    /// The control cannot create a tab on its own — the page the new tab would show
    /// belongs to the host — so the gesture is reported rather than acted on. A host
    /// with no subscription therefore sees no behaviour change (ADV-009).
    pub new_tab_requested: GenericSignal,
    /// The host-side hover/delay/show pipeline for tab tooltips (ADV-11).
    ///
    /// `TabBarTab::tooltip` was stored, published and never read, so a truncated title — the very
    /// case a hint exists for — could not be read anywhere. The pipeline lives in the control
    /// because a widget cannot reach a host's manager instance; the shape is the one `tool_bar`
    /// already uses.
    tooltips: TooltipManager,
    /// The derived hover id currently registered with the tooltip pipeline.
    tooltip_source: Option<ObjectId>,
    /// Monotonic clock fed to the tooltip pipeline from [`Widget::tick`].
    tooltip_now_ms: u64,
}

/// The resolved chrome colours shared by every tab in one draw pass.
///
/// Split out of `draw_tab` so the six values (and the two `WidgetStyle` resolutions they
/// derive from) are computed once per frame rather than once per tab. `Copy` because they are
/// all `Color`.
#[derive(Clone, Copy)]
struct TabChrome {
    border: Color,
    text_color: Color,
    current_tab: Color,
    disabled_tab: Color,
    inactive_tab: Color,
    hovered_tab: Color,
}

impl TabBar {
    /// Creates a new `TabBar` with the given geometry.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::TabBar, geometry, "TabBar"),
            tabs: Vec::new(),
            current_index: None,
            hovered_index: None,
            tab_position: TabPosition::North,
            tab_shape: TabShape::Rounded,
            closable: false,
            movable: false,
            dragging_from: None,
            tab_min_width: TAB_MIN_WIDTH,
            tab_max_width: TAB_MAX_WIDTH,
            measured_title_widths: Vec::new(),
            measured_title_signature: 0,
            elided_titles: Vec::new(),
            direction: crate::core::TextDirection::default(),
            drag_target: None,
            scroll_buttons: false,
            scroll_offset: 0,
            elide_mode: TabElideMode::Right,
            auto_hide: false,
            expanding: false,
            selection_behavior_on_remove: SelectionBehaviorOnRemove::default(),
            style_variant: TabBarStyle::Standard,
            tab_bar_clicked: Signal1::new(),
            current_changed: Signal1::new(),
            tab_close_requested: Signal1::new(),
            tab_moved: Signal1::new(),
            new_tab_requested: GenericSignal::new(),
            tooltips: TooltipManager::new(geometry),
            tooltip_source: None,
            tooltip_now_ms: 0,
        }
    }

    // ---------------------------------------------------------------------------
    // Tab management
    // ---------------------------------------------------------------------------

    /// Adds a tab with the given title and returns the index of the new tab.
    pub fn add_tab(&mut self, title: String) -> usize {
        self.tabs.push(TabBarTab::new(title));
        let idx = self.tabs.len().saturating_sub(1);
        if self.current_index.is_none() {
            self.current_index = Some(idx);
        }
        idx
    }

    /// Assigns a typed icon source to an existing tab.
    pub fn set_tab_icon_source(
        &mut self,
        index: usize,
        source: Option<IconSource>,
    ) -> Result<(), IconItemError> {
        let len = self.tabs.len();
        let Some(tab) = self.tabs.get_mut(index) else {
            return Err(IconItemError::IndexOutOfBounds { index, len });
        };
        tab.set_icon_source(source);
        self.base.request_redraw();
        Ok(())
    }

    /// Returns the tab at `index`.
    ///
    /// The same accessor pair [`TabWidget`](crate::widget::TabWidget) offers, so the crate's two tab
    /// strips are read and written the same way. Without it a tab's legacy `Image` icon had **no**
    /// reachable path at all: the field existed, `TabBarTab::set_icon` existed, and no `TabBar` API
    /// could ever call it.
    pub fn tab(&self, index: usize) -> Option<&TabBarTab> {
        self.tabs.get(index)
    }

    /// Returns the tab at `index`, mutably.
    pub fn tab_mut(&mut self, index: usize) -> Option<&mut TabBarTab> {
        self.tabs.get_mut(index)
    }

    /// Inserts a tab at the given index.
    pub fn insert_tab(&mut self, index: usize, title: String) {
        let idx = index.min(self.tabs.len());
        self.tabs.insert(idx, TabBarTab::new(title));
        // Adjust current_index if needed.
        if let Some(cur) = self.current_index {
            if cur >= idx {
                self.current_index = Some(cur + 1);
            }
        } else {
            self.current_index = Some(idx);
        }
    }

    /// Removes the tab at the given index.
    pub fn remove_tab(&mut self, index: usize) {
        if index >= self.tabs.len() {
            return;
        }
        self.tabs.remove(index);
        // Adjust current_index.
        match self.current_index {
            Some(cur) if cur == index => {
                if self.tabs.is_empty() {
                    self.current_index = None;
                } else {
                    let next = match self.selection_behavior_on_remove {
                        SelectionBehaviorOnRemove::SelectPrevious => cur.saturating_sub(1),
                        SelectionBehaviorOnRemove::SelectNext => cur.min(self.tabs.len() - 1),
                    };
                    self.current_index = Some(next);
                }
            }
            Some(cur) if cur > index => {
                self.current_index = Some(cur - 1);
            }
            // No action needed for this transition
            _ => {}
        }
    }

    /// Removes all tabs.
    pub fn clear(&mut self) {
        self.tabs.clear();
        self.current_index = None;
        self.hovered_index = None;
    }

    /// Returns the number of tabs.
    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns the number of tabs.
    pub fn count(&self) -> usize {
        self.tabs.len()
    }

    /// Returns `true` if there are no tabs.
    pub fn is_empty(&self) -> bool {
        self.tabs.is_empty()
    }

    // ---------------------------------------------------------------------------
    // Tab text
    // ---------------------------------------------------------------------------

    /// Returns the text of the tab at the given index.
    pub fn tab_text(&self, index: usize) -> Option<&str> {
        self.tabs.get(index).map(|t| t.title.as_str())
    }

    /// Sets the text of the tab at the given index.
    /// Renames the tab at `index`.
    ///
    /// Repaints: the title is what the strip draws (`draw` measures and elides each tab from its
    /// own `title`), so a rename that did not mark damage left the old caption on screen.
    pub fn set_tab_text(&mut self, index: usize, text: String) {
        let Some(tab) = self.tabs.get_mut(index) else {
            return;
        };
        if tab.title == text {
            return;
        }
        tab.title = text;
        self.base.request_redraw();
    }

    // ---------------------------------------------------------------------------
    // Tab enabled state
    // ---------------------------------------------------------------------------

    /// Returns whether the tab at the given index is enabled.
    pub fn tab_enabled(&self, index: usize) -> Option<bool> {
        self.tabs.get(index).map(|t| t.enabled)
    }

    /// Sets the enabled state of the tab at the given index.
    pub fn set_tab_enabled(&mut self, index: usize, enabled: bool) {
        if let Some(tab) = self.tabs.get_mut(index) {
            tab.enabled = enabled;
        }
    }

    // ---------------------------------------------------------------------------
    // Current index
    // ---------------------------------------------------------------------------

    /// Returns the current tab index.
    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    /// Sets the current tab index.
    ///
    /// An index outside the tab list is ignored; use [`Self::clear_current_index`]
    /// to deliberately select nothing.
    pub fn set_current_index(&mut self, index: usize) {
        if index < self.tabs.len() {
            let changed = self.current_index != Some(index);
            self.current_index = Some(index);
            if changed {
                self.current_changed.emit(index);
                self.base.request_redraw();
            }
        }
    }

    /// Clears the selection so no tab is current.
    ///
    /// The property contract needs this: `current_index` reads as `Null` when
    /// nothing is selected, so a writer must be able to restore that state or the
    /// read → write → read round-trip would not close.
    pub fn clear_current_index(&mut self) {
        if self.current_index.take().is_some() {
            self.base.changed.emit();
            self.base.request_redraw();
        }
    }

    // ---------------------------------------------------------------------------
    // Tab position
    // ---------------------------------------------------------------------------

    /// Returns the tab position.
    pub fn tab_position(&self) -> TabPosition {
        self.tab_position
    }

    /// Sets the tab position. Requests redraw if the position changes.
    pub fn set_tab_position(&mut self, position: TabPosition) {
        if self.tab_position != position {
            self.tab_position = position;
            self.base.request_redraw();
        }
    }

    /// Returns the writing direction the horizontal strip runs in.
    pub fn direction(&self) -> crate::core::TextDirection {
        self.direction
    }

    /// Sets the writing direction the horizontal strip runs in, and repaints.
    ///
    /// A right-to-left strip starts at its **right** edge, so tab 0 and the overflow both grow
    /// leftward — the visual order then matches the order of the pages the tabs select. The vertical
    /// positions are unaffected; see the field for why the side a strip is attached to is not a
    /// reading fact.
    pub fn set_direction(&mut self, direction: crate::core::TextDirection) {
        if self.direction != direction {
            self.direction = direction;
            self.base.request_redraw();
        }
    }

    // ---------------------------------------------------------------------------
    // Tab shape
    // ---------------------------------------------------------------------------

    /// Returns the tab shape.
    pub fn tab_shape(&self) -> TabShape {
        self.tab_shape
    }

    /// Sets the tab shape. Requests redraw.
    pub fn set_tab_shape(&mut self, shape: TabShape) {
        if self.tab_shape != shape {
            self.tab_shape = shape;
            self.base.request_redraw();
        }
    }

    // ---------------------------------------------------------------------------
    // Closable / Movable
    // ---------------------------------------------------------------------------

    /// Returns whether tabs are closable.
    pub fn closable(&self) -> bool {
        self.closable
    }

    /// Sets whether tabs should show a close button.
    /// Enables or disables the per-tab close buttons, and repaints.
    ///
    /// The close button is drawn from this flag (`draw`'s `// Draw close button if closable`),
    /// so a caller that switched it on saw nothing until an unrelated event repainted the strip.
    pub fn set_closable(&mut self, closable: bool) {
        if self.closable == closable {
            return;
        }
        self.closable = closable;
        self.base.request_redraw();
    }

    /// Returns whether tabs are movable.
    pub fn movable(&self) -> bool {
        self.movable
    }

    /// Sets whether tabs can be moved via drag-and-drop.
    /// Enables or disables drag-to-reorder, and repaints.
    ///
    /// `movable` gates the press arm that arms a drag, and the draw path shows the drag
    /// affordance, so the flag is both painted and hit-tested — toggling it has to mark damage.
    pub fn set_movable(&mut self, movable: bool) {
        if self.movable == movable {
            return;
        }
        self.movable = movable;
        self.base.request_redraw();
    }

    /// Moves the tab at `from` so that it sits at `to`, and emits
    /// [`Self::tab_moved`] with `(from, to)`.
    ///
    /// # Why this is the only way to reorder
    ///
    /// `tab_moved` was declared but nothing emitted it, so a caller could see the
    /// signal, connect a handler, and wait forever. The fix is not to emit from
    /// somewhere convenient but to give reordering a real entry point: this is that
    /// entry point, and the drag gesture calls it.
    ///
    /// # Why `to` is clamped rather than rejected
    ///
    /// A drag reports positions from pointer coordinates, which can land one past
    /// the last tab. Clamping to `tabs.len() - 1` treats "dropped past the end" as
    /// "moved to the end" — the intent — while an out-of-range `from` is a caller
    /// bug and is rejected without emitting.
    ///
    /// Returns `true` when a move happened.
    pub fn move_tab(&mut self, from: usize, to: usize) -> bool {
        if from >= self.tabs.len() {
            return false;
        }
        let to = to.min(self.tabs.len() - 1);
        if to == from {
            return false;
        }

        let tab = self.tabs.remove(from);
        self.tabs.insert(to, tab);

        // The selection follows the *tab*, not the slot: dragging the current tab
        // must not silently move the selection to whichever tab took its index.
        if let Some(current) = self.current_index {
            self.current_index = Some(match current {
                c if c == from => to,
                c if from < c && c <= to => c - 1,
                c if to <= c && c < from => c + 1,
                c => c,
            });
        }

        self.tab_moved.emit((from, to));
        true
    }

    // ---------------------------------------------------------------------------
    // Tab dimensions
    // ---------------------------------------------------------------------------

    /// The tab index the pointer is currently dragging over, while a reorder is in
    /// flight.
    pub fn drag_target(&self) -> Option<usize> {
        self.drag_target
    }

    /// Whether a drag is in flight.
    pub fn is_dragging(&self) -> bool {
        self.dragging_from.is_some()
    }

    /// Whether the strip paints scroll buttons when its tabs overflow.
    pub fn has_scroll_buttons(&self) -> bool {
        self.scroll_buttons
    }

    /// Enables the overflow scroll buttons. Off by default, so a strip keeps its
    /// historical shrink-to-fit behaviour.
    pub fn set_scroll_buttons(&mut self, enabled: bool) {
        if self.scroll_buttons == enabled {
            return;
        }
        self.scroll_buttons = enabled;
        if !enabled {
            self.scroll_offset = 0;
        }
        self.base.request_redraw();
    }

    /// Whether the tabs are wider than the strip at their minimum width.
    pub fn is_overflowing(&self) -> bool {
        let count = self.tabs.len();
        if count == 0 {
            return false;
        }
        let needed = count as u32 * self.tab_min_width
            + (count.saturating_sub(1)) as u32 * TAB_SPACING as u32;
        needed > self.base.geometry().width
    }

    /// The scroll buttons' boxes, when they are enabled and needed.
    pub fn scroll_button_rects(&self) -> Option<(Rect, Rect)> {
        if !self.scroll_buttons || !self.is_overflowing() {
            return None;
        }
        let g = self.base.geometry();
        let width = 16u32.min(g.width);
        Some((
            Rect::new(g.x, g.y, width, TAB_HEIGHT as u32),
            Rect::new(g.x + g.width as i32 - width as i32, g.y, width, TAB_HEIGHT as u32),
        ))
    }

    /// How many tabs the scroll buttons have shifted the run.
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Shifts the visible run by `delta` tabs, clamped to the list.
    pub fn scroll_by(&mut self, delta: i32) -> bool {
        if !self.is_overflowing() {
            return false;
        }
        let max = self.max_scroll_offset();
        let next = (self.scroll_offset as i32 + delta).clamp(0, max as i32) as usize;
        if next == self.scroll_offset {
            return false;
        }
        self.scroll_offset = next;
        self.base.request_redraw();
        true
    }

    /// Registers the hovered tab's hint with the hover pipeline (ADV-11).
    ///
    /// The text precedence is the issue's rule: an explicit tooltip wins; otherwise a title that
    /// was actually elided shows its full text, because the truncation is what hides it. A tab
    /// whose title fits and has no tooltip registers nothing.
    fn sync_hover_tooltip(&mut self) {
        let Some(index) = self.hovered_index else {
            if self.tooltip_source.take().is_some() {
                self.tooltips.note_leave();
            }
            return;
        };
        let elided = matches!(
            self.elided_titles.get(index).and_then(Option::as_ref).map(|(_, entry)| entry),
            Some(ElidedTitle::Elided(_))
        );
        let Some(tab) = self.tabs.get(index) else {
            if self.tooltip_source.take().is_some() {
                self.tooltips.note_leave();
            }
            return;
        };
        let text = if !tab.tooltip().is_empty() {
            Some(tab.tooltip().to_string())
        } else if elided {
            Some(tab.title().to_string())
        } else {
            None
        };
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

    /// The largest scroll offset the strip allows.
    fn max_scroll_offset(&self) -> usize {
        let g = self.base.geometry();
        let available = g.width.saturating_sub(32) as usize;
        let tab = self.tab_min_width.max(1) as usize + TAB_SPACING as usize;
        let visible = (available / tab).max(1);
        self.tabs.len().saturating_sub(visible)
    }

    /// How a title that does not fit is handled.
    pub fn elide_mode(&self) -> TabElideMode {
        self.elide_mode
    }

    /// Sets the title elision mode; `Right` is the historical behaviour.
    pub fn set_elide_mode(&mut self, mode: TabElideMode) {
        if self.elide_mode == mode {
            return;
        }
        self.elide_mode = mode;
        self.base.request_redraw();
    }

    /// Whether the strip hides itself with at most one tab.
    pub fn auto_hide(&self) -> bool {
        self.auto_hide
    }

    /// Enables auto-hide. Off by default.
    pub fn set_auto_hide(&mut self, enabled: bool) {
        if self.auto_hide == enabled {
            return;
        }
        self.auto_hide = enabled;
        self.base.request_redraw();
    }

    /// Whether the tabs stretch to fill the strip.
    pub fn expanding(&self) -> bool {
        self.expanding
    }

    /// Enables expanding tabs. Off by default, so a tab keeps its natural width.
    pub fn set_expanding(&mut self, enabled: bool) {
        if self.expanding == enabled {
            return;
        }
        self.expanding = enabled;
        self.base.request_redraw();
    }

    /// What becomes current when the current tab is removed.
    pub fn selection_behavior_on_remove(&self) -> SelectionBehaviorOnRemove {
        self.selection_behavior_on_remove
    }

    /// Sets the post-removal selection rule.
    pub fn set_selection_behavior_on_remove(&mut self, behavior: SelectionBehaviorOnRemove) {
        self.selection_behavior_on_remove = behavior;
    }

    /// The strip's visual variant.
    pub fn style_variant(&self) -> TabBarStyle {
        self.style_variant
    }

    /// Sets the strip's visual variant; `Standard` is the historical rendering.
    pub fn set_style_variant(&mut self, variant: TabBarStyle) {
        if self.style_variant == variant {
            return;
        }
        self.style_variant = variant;
        self.base.request_redraw();
    }

    /// Returns the minimum tab width.
    pub fn tab_min_width(&self) -> u32 {
        self.tab_min_width
    }

    /// Sets the minimum tab width.
    ///
    /// Raises the maximum to match if the new minimum would exceed it, mirroring
    /// [`Self::set_tab_max_width`]. Without that the pair could be left inverted, and
    /// `Self::compute_tab_width``s `clamp(min, max)` **panics** on `min > max` — reaching it via
    /// `set_tab_min_width(5000)` on a bar whose maximum was still the 200 default. The two setters
    /// were asymmetric: `set_tab_max_width` already pulled the maximum up to the minimum, this one
    /// left the maximum behind.
    /// Sets the minimum tab width, raising the maximum with it if needed, and repaints.
    ///
    /// Every tab's width is clamped into `min..=max`, so this changes the painted run rather than
    /// only a number a caller reads back.
    pub fn set_tab_min_width(&mut self, width: u32) {
        let width = width.max(1);
        let raised_max = self.tab_max_width.max(width);
        if self.tab_min_width == width && self.tab_max_width == raised_max {
            return;
        }
        self.tab_min_width = width;
        self.tab_max_width = raised_max;
        self.base.request_redraw();
    }

    /// Returns the maximum tab width.
    pub fn tab_max_width(&self) -> u32 {
        self.tab_max_width
    }

    /// Sets the maximum tab width.
    ///
    /// Floors at the current minimum, so the pair the clamp is applied to is never inverted.
    ///
    /// # The asymmetry this fixes
    ///
    /// [`Self::set_tab_min_width`] repaints; this one did not, though the two bounds are the same
    /// fact seen from either end and both feed `compute_tab_width`. So raising a tab's minimum
    /// re-laid-out the bar immediately, while lowering its maximum — the gesture a user makes to
    /// fit *more* tabs in — changed every tab's width on the next unrelated frame instead.
    pub fn set_tab_max_width(&mut self, width: u32) {
        let clamped = width.max(self.tab_min_width);
        if self.tab_max_width == clamped {
            return;
        }
        self.tab_max_width = clamped;
        self.base.request_redraw();
    }

    // ---------------------------------------------------------------------------
    // Internal helpers
    // ---------------------------------------------------------------------------

    /// Returns the rect for the tab at the given index.
    ///
    /// # Why the width is scaled before the position is derived
    ///
    /// The position used to be `(tab_width + spacing) * index`, which never mentioned
    /// `rect.width`. With six tabs at 200 px on a 240 px strip the fifth began at x = 808 and the
    /// sixth at x = 1010 — both drawn entirely outside the control. The clamp below is the rule
    /// [`crate::widget::nav_widgets::tab_view`] already applies: when the tabs do not fit, they
    /// shrink to `available / count` and every tab is on screen. The *spacing* has to be inside
    /// the fit calculation for the same reason: `count` tabs need `count` widths plus
    /// `count - 1` gaps, and leaving the gaps out is how "they fit now" becomes false again one
    /// tab later.
    fn tab_rect(&self, index: usize) -> Option<Rect> {
        if index >= self.tabs.len() {
            return None;
        }
        let rect = self.base.geometry();
        let spacing = TAB_SPACING;
        // The strip's *reading* order decides which end tab 0 is at, and the direction is applied to
        // the finished offset rather than to the index: `offset` is "how far into the strip this tab
        // begins in reading order", and the conversion to a left-edge distance is
        // `TextDirection`'s single job. Doing it the other way — reversing the index — would also
        // reverse the overflow, so the tabs would grow off the *wrong* end once they stopped fitting.
        let rtl = self.direction.is_right_to_left();
        match self.tab_position {
            TabPosition::North | TabPosition::South => {
                let tab_width = self.fitted_tab_width();
                let shift = if self.scroll_buttons && self.is_overflowing() {
                    16 + self.scroll_offset as i32 * (self.tab_min_width as i32 + spacing)
                } else {
                    0
                };
                let offset = (tab_width as i32 + spacing) * index as i32 - shift;
                let x =
                    if rtl { rect.right() - offset - tab_width as i32 } else { rect.x + offset };
                // The last tab must end *inside* the strip. `fitted_tab_width` divides by the count
                // and subtracts the whole gap budget, so a tab entirely on screen needs the whole
                // run to fit; the clamp still holds for a tab whose own measured width is larger
                // than its slot, and it is what keeps the assertion of that visible rather than
                // implicit.
                //
                // In RTL the tab's **leading** edge is on its right, so the room it may take is
                // measured from the strip's left edge — the mirror of the LTR clamp, not a second
                // rule.
                let width = if rtl {
                    tab_width.min((x + tab_width as i32 - rect.x).max(0) as u32)
                } else {
                    tab_width.min((rect.right() - x).max(0) as u32)
                };
                let y = if self.tab_position == TabPosition::North {
                    rect.y
                } else {
                    rect.y + rect.height as i32 - TAB_HEIGHT
                };
                Some(Rect::new(x, y, width, TAB_HEIGHT as u32))
            }
            TabPosition::West | TabPosition::East => {
                let tab_height = TAB_HEIGHT as u32;
                let y = rect.y + (TAB_HEIGHT + spacing) * index as i32;
                // The side a vertical strip is attached to is a layout choice, not a reading one, so
                // the direction does not move it: a `West` strip stays on the left in every locale,
                // and its tabs still run top-to-bottom.
                let x = if self.tab_position == TabPosition::West {
                    rect.x
                } else {
                    rect.x + rect.width as i32 - TAB_HEIGHT
                };
                Some(Rect::new(x, y, TAB_HEIGHT as u32, tab_height))
            }
        }
    }

    /// The width every tab is drawn at, after shrinking to fit the strip when it has to.
    ///
    /// # Why the fit is applied here rather than per tab
    ///
    /// [`Self::compute_tab_width`] answers "how wide does this tab's own label want to be",
    /// which is a per-tab question; whether the whole *strip* fits is not. Deciding it in one
    /// place means every tab is scaled by the same rule, so a strip never mixes fitted and
    /// unfitted tabs — which would show as a tab whose label is elided next to one that is not.
    fn fitted_tab_width(&self) -> u32 {
        let rect = self.base.geometry();
        let count = self.tabs.len();
        if count == 0 {
            return 0;
        }
        let count = count as u32;
        let gaps = (TAB_SPACING as u32).saturating_mul(count.saturating_sub(1));
        let available = rect.width.saturating_sub(gaps).max(1);
        if self.expanding {
            return (available / count).max(1);
        }
        let natural = self.compute_tab_width(0).max(self.tab_min_width).max(1);
        natural.min(available / count).max(1)
    }

    /// The room a tab's title has inside the tab it is painted in.
    ///
    /// Mirrors the reservation [`Self::compute_tab_width`] makes: one text inset at each
    /// end, plus the leading icon slot and the trailing close button when they are drawn.
    /// The strip's fitted width can be narrower than that reservation, which is why the
    /// title has to be measured against the *actual* tab rect rather than the natural one.
    fn title_band_width(tab_rect: Rect, has_icon: bool, closable: bool) -> u32 {
        let leading = TAB_TEXT_INSET + if has_icon { TAB_ICON_SIZE + TAB_ICON_GAP } else { 0 };
        let trailing =
            TAB_TEXT_INSET + if closable { CLOSE_SIZE as u32 + CLOSE_PADDING as u32 } else { 0 };
        tab_rect.width.saturating_sub(leading + trailing)
    }

    /// The title as it is painted: elided to the tab's own label band when the strip has
    /// shrunk the tab below what the label needs.
    ///
    /// `measure` is the renderer's own advance measurement, so the budget this decides on
    /// is the width the paint path will actually produce. A title that fits is returned
    /// borrowed — the common case — so painting it stays allocation-free.
    fn fitted_title<'a>(
        &'a self,
        index: usize,
        tab_rect: Rect,
        measure: &mut dyn FnMut(&str) -> u32,
    ) -> Cow<'a, str> {
        if self.elide_mode == TabElideMode::None {
            return Cow::Borrowed(&self.tabs[index].title);
        }
        let has_icon = self.tabs[index].has_icon();
        let budget = Self::title_band_width(tab_rect, has_icon, self.closable);
        let title = self.tabs[index].title.as_str();
        match self.elide_mode {
            TabElideMode::None => Cow::Borrowed(title),
            TabElideMode::Right => elide_to_width(title, budget, measure),
            TabElideMode::Left => elide_start_to_width(title, budget, measure),
            TabElideMode::Middle => {
                crate::widget::text_utils::elide_middle_to_width(title, budget, measure)
            }
        }
    }

    /// A fold over every input `measured_title_widths` is measured from (K-P09).
    fn measured_title_signature(&self, font: &Font) -> u64 {
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |value: u64| {
            hash ^= value;
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        };
        mix(u64::from(font.size().to_bits()));
        mix(u64::from(font.weight()));
        for byte in font.family().as_bytes() {
            mix(u64::from(*byte));
        }
        mix(self.tabs.len() as u64);
        for tab in &self.tabs {
            for byte in tab.title.as_bytes() {
                mix(u64::from(*byte));
            }
            // A title terminator, so "ab"+"c" cannot hash as "a"+"bc".
            mix(0xFF);
        }
        hash
    }

    /// Rebuilds [`Self::measured_title_widths`] only when the font or a title changed (K-P09).
    fn refresh_measured_title_widths(&mut self, font: &Font, context: &RenderContext) {
        let signature = self.measured_title_signature(font);
        if self.measured_title_signature == signature
            && self.measured_title_widths.len() == self.tabs.len()
        {
            return;
        }
        self.measured_title_widths.clear();
        for tab in &self.tabs {
            let width = context.measure_text(&tab.title, font).width;
            self.measured_title_widths.push(width);
        }
        self.measured_title_signature = signature;
    }

    /// Refreshes the cached elided title for one tab, if its inputs changed (K-P09).
    fn refresh_elided_title(
        &mut self,
        index: usize,
        tab_rect: Rect,
        font: &Font,
        measure: &mut dyn FnMut(&str) -> u32,
    ) {
        // Borrowed for the key fold and then released: cloning the title here would put back the
        // per-frame allocation the cache exists to remove (K-P09).
        let key = {
            let tab = &self.tabs[index];
            let budget = Self::title_band_width(tab_rect, tab.has_icon(), self.closable);
            elide_key(&tab.title, budget, font, self.elide_mode)
        };
        if self.elided_titles.len() <= index {
            self.elided_titles.resize_with(index + 1, || None);
        }
        if let Some((cached_key, _)) = self.elided_titles[index].as_ref() {
            if *cached_key == key {
                return;
            }
        }
        // A vertical strip is exempt from eliding: its tabs are one text line wide, so a width
        // budget there would blank every title.
        let entry = if matches!(self.tab_position, TabPosition::West | TabPosition::East) {
            ElidedTitle::Fits
        } else {
            match self.fitted_title(index, tab_rect, measure) {
                Cow::Borrowed(_) => ElidedTitle::Fits,
                Cow::Owned(text) => ElidedTitle::Elided(text),
            }
        };
        self.elided_titles[index] = Some((key, entry));
    }

    /// The title to paint for tab `index`: the cached elided form, or the tab's own title.
    fn painted_title(&self, index: usize) -> &str {
        match self.elided_titles.get(index).and_then(Option::as_ref).map(|(_, entry)| entry) {
            Some(ElidedTitle::Elided(text)) => text.as_str(),
            _ => self.tabs[index].title.as_str(),
        }
    }

    /// Computes the width of a tab, taking into account whether a close button
    /// is rendered and clamping to the configured min/max.
    ///
    /// # Why the estimate cannot be "characters x 8"
    ///
    /// The estimate used to be `text.len() * 8`, which is wrong twice over: `len()`
    /// counts **bytes**, so a two-character CJK title measured 48 px; and the advance
    /// it should predict is the one the *renderer* will use, which is 12 px per glyph
    /// (`Font::simple("Arial", 14.0)` in [`Self::draw_tab`]). A title that measured
    /// wider here than it drew there left the label running past its own tab; one that
    /// measured narrower wasted the tab.
    ///
    /// The fallback therefore mirrors `RenderContext`'s own heuristic — one cluster per
    /// `char`, each advancing by the font size — rather than inventing a second one. It
    /// is only reached before the first `draw`, because `draw_tab` records the measured
    /// width in [`Self::measured_title_widths`] from then on.
    /// The font tab labels are drawn and measured with: the resolved theme/caller font at the
    /// tab's role size, so the title honours the theme body font and the user's text scale
    /// (D09-STYLE-01).
    ///
    /// Named once so `compute_tab_width`, `draw_tab` and `draw` all measure with the font the
    /// labels are painted with — a reserved width and its ink cannot disagree.
    fn label_font(&self) -> Font {
        role_font(self.style(), TAB_FONT_SIZE)
    }

    fn compute_tab_width(&self, index: usize) -> u32 {
        let text = self.tabs[index].title.as_str();
        let measured = self.measured_title_widths.get(index).copied();
        let text_width = measured.unwrap_or_else(|| estimate_text_width(text, &self.label_font()));
        let icon_width = if self.tabs[index].has_icon() { TAB_ICON_SIZE + TAB_ICON_GAP } else { 0 };
        let mut w = text_width + TAB_TEXT_PADDING + icon_width;
        if self.closable {
            w += (CLOSE_SIZE + CLOSE_PADDING) as u32;
        }
        w.clamp(self.tab_min_width, self.tab_max_width)
    }

    /// Roughly measure text width (fallback if no RenderContext handy).
    /// Moves the selection by `delta` tabs, skipping disabled tabs.
    fn step_selection(&mut self, delta: i32) {
        let count = self.tabs.len();
        if count == 0 || delta == 0 {
            return;
        }
        let start = match self.current_index {
            Some(index) => index as i32,
            None if delta > 0 => -1,
            None => count as i32,
        };
        let mut index = start;
        for _ in 0..count {
            index = (index + delta).rem_euclid(count as i32);
            if self.tabs[index as usize].enabled {
                self.set_current_index(index as usize);
                return;
            }
        }
    }

    /// The index of the tab at the given point, or None.
    fn tab_at_position(&self, pos: Point) -> Option<usize> {
        for i in 0..self.tabs.len() {
            if let Some(r) = self.tab_rect(i) {
                if r.contains(pos) {
                    return Some(i);
                }
            }
        }
        None
    }

    /// Returns the close-button rect for the tab at the given index, if closable.
    fn close_rect(&self, index: usize) -> Option<Rect> {
        if !self.closable {
            return None;
        }
        self.tab_rect(index).map(|r| {
            let cx = r.x + r.width as i32 - CLOSE_SIZE - CLOSE_PADDING;
            let cy = r.y + (r.height as i32 - CLOSE_SIZE) / 2;
            Rect::new(cx, cy, CLOSE_SIZE as u32, CLOSE_SIZE as u32)
        })
    }

    /// The region a press must land in to close the tab at `index`.
    ///
    /// The painted glyph is only [`CLOSE_SIZE`] square, well below the platform's minimum
    /// hit size, so [`expand_to_hit_area`] widens it instead of demanding pixel-accurate
    /// aiming at a 12 px mark. The widened region is then clipped to the tab that owns the
    /// glyph: without that, reaching for the next tab's leading edge would close this one.
    fn close_hit_area(&self, index: usize) -> Option<Rect> {
        let close = self.close_rect(index)?;
        let tab = self.tab_rect(index)?;
        let expanded = expand_to_hit_area(close);
        Some(expanded.intersection(&tab).unwrap_or(close))
    }

    /// The rectangle the keyboard focus ring follows.
    ///
    /// The ring belongs to the tab the keyboard is on — the current one — because a strip
    /// has no single interior an outline could meaningfully follow. A strip with no
    /// selection has no tab to ring, so its own rectangle is the fallback; keyboard focus
    /// must never be invisible.
    fn focus_ring_rect(&self) -> Rect {
        self.current_index
            .and_then(|index| self.tab_rect(index))
            .unwrap_or_else(|| self.base.geometry())
    }

    /// The corner radius the focus ring follows for the active tab shape.
    ///
    /// `FocusRing::for_control` tightens this by the ring width again, so the ring traces
    /// the tab's own silhouette rather than a fixed shape that disagrees with it.
    fn focus_ring_corner(&self) -> u32 {
        match self.tab_shape {
            TabShape::Rounded => 4,
            TabShape::Triangular | TabShape::Rectangular => 0,
        }
    }

    /// The chrome shared by every tab in the strip.
    ///
    /// Resolved once per `draw` rather than once per tab: the six derived colours do not
    /// depend on the tab, so resolving them inside `draw_tab` cloned the resolved
    /// `WidgetStyle` twice for every tab on every frame (the style carries a font-family
    /// `String` and a gradient stop vector).
    fn tab_chrome(&self) -> TabChrome {
        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then a literal. The theme step is what
        // makes an appearance switch visible; previously every colour below was a
        // hardcoded literal, so light and dark rendered identically.
        //
        // `resolved_theme_style` takes and releases the global manager's lock
        // internally, so no guard is held across the draw (the mutex is not
        // re-entrant).
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("tab_bar");
        // `tab_bar` is not a control kind in the role table, so it classifies as
        // `Surface`, whose background is `theme.colors.background` — byte-identical
        // to the window behind it. The current tab's fill is therefore a step toward
        // the foreground, so the strip reads as a surface of its own.
        let resolved = style
            .background_color
            .or_else(|| theme.as_ref().and_then(|t| t.background_color))
            .unwrap_or(Color::rgb(255, 255, 255));
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or(Color::rgb(180, 180, 180));
        let text_color = style
            .text_color
            .or_else(|| theme.as_ref().and_then(|t| t.text_color))
            .unwrap_or(Color::rgb(0, 0, 0));
        let current_tab = resolved.blend(&text_color, 0.08);
        // Tab chrome is derived from that pair: a disabled tab is pressed toward the
        // foreground, an inactive one less so, and a hovered tab reads as a light
        // tint of the selection colour.
        let disabled_tab = current_tab.blend(&text_color, 0.14);
        let inactive_tab = current_tab.blend(&text_color, 0.06);
        let hovered_tab = current_tab.blend(
            &crate::style::resolved_theme_style_shared("button")
                .and_then(|button| button.background_color)
                .unwrap_or(text_color),
            0.08,
        );
        TabChrome { border, text_color, current_tab, disabled_tab, inactive_tab, hovered_tab }
    }

    /// Draws a single tab with the given shape.
    fn draw_tab(
        &self,
        context: &mut RenderContext,
        index: usize,
        tab_rect: Rect,
        chrome: &TabChrome,
        font: &Font,
        title: &str,
        title_height: i32,
    ) {
        let tab = &self.tabs[index];
        let is_current = self.current_index == Some(index);
        let is_hovered = self.hovered_index == Some(index);
        let is_enabled = tab.enabled;

        let border = chrome.border;
        let text_color = chrome.text_color;
        let current_tab = chrome.current_tab;
        let disabled_tab = chrome.disabled_tab;
        let inactive_tab = chrome.inactive_tab;
        let hovered_tab = chrome.hovered_tab;

        // Background color.
        let bg = if !is_enabled {
            disabled_tab
        } else if self.dragging_from == Some(index) {
            // The dragged tab is carried, not resting: a tint states that it is the
            // one that will move.
            current_tab.blend(&chrome.current_tab.contrast_color(), 0.12)
        } else if is_current {
            if self.style_variant == TabBarStyle::Segmented {
                current_tab.blend(&chrome.current_tab.contrast_color(), 0.22)
            } else {
                current_tab
            }
        } else if is_hovered {
            hovered_tab
        } else {
            inactive_tab
        };

        // Border color.
        let border =
            if !is_enabled || is_current { border } else { border.blend(&inactive_tab, 0.5) };

        // Draw tab shape.
        //
        // The three `TabShape` values were drawn *identically* — all three arms were
        // `fill_rect` + `draw_rect` — so the property moved a value and nothing on screen, and
        // the `Triangular` arm's comment claimed top corners were clipped by work that was never
        // done. The shapes here are the ones [`crate::widget::container_widgets::tabwidget`]
        // draws for the same enum: the two tab controls agree about what `TabShape` means, which
        // is what lets a theme or a caller move a strip from one to the other unchanged.
        //
        // `Rectangular` is the bottom arm because it is the plain case, so a shape added later
        // cannot silently take it over the way the wildcard arm took over `Rectangular` before.
        match self.tab_shape {
            TabShape::Rounded => {
                let radius = 4;
                context.fill_rounded_rect(tab_rect, radius, bg);
                context.draw_rounded_rect_stroke(tab_rect, radius, border, 1);
                // The current tab shares its edge with what it labels, so its bottom edge is
                // overdrawn with the tab's own fill to hide the outline there — the same
                // "blends with the content area" idea `tabwidget` gets for free because its
                // current tab *is* painted in the content colour.
                if is_current {
                    let bottom_rect = Rect::new(
                        tab_rect.x + 1,
                        tab_rect.y + tab_rect.height as i32 - 1,
                        tab_rect.width.saturating_sub(2),
                        2,
                    );
                    context.fill_rect(bottom_rect, bg);
                }
            }
            TabShape::Triangular => {
                // A real triangle: apex on the top edge at the tab's centre, base along the
                // bottom edge of the strip. The comment used to say this and the code did not do
                // it — it filled the whole rectangle and claimed the corners were "clipped".
                let apex_x = tab_rect.x + tab_rect.width as i32 / 2;
                let base_y = tab_rect.y + tab_rect.height as i32;
                let points = [
                    Point::new(apex_x, tab_rect.y),
                    Point::new(tab_rect.x, base_y),
                    Point::new(tab_rect.x + tab_rect.width as i32, base_y),
                ];
                context.draw_path(&points, true, bg, true, 0);
                context.draw_path(&points, true, border, false, 1);
            }
            TabShape::Rectangular => {
                context.fill_rect(tab_rect, bg);
                context.draw_rect(tab_rect, border);
            }
        }

        // Text color. A disabled tab's label recedes from its own fill by the shared
        // disabled weight, rather than by a local blend toward a literal grey.
        let text_color = self.base.disabled_ink_on(text_color, disabled_tab);

        // Draw tab title. The origin is the glyph's **top** edge, so the vertical centre is
        // half the difference between the tab and the line box. Passing the tab's midline
        // (which is what `tab_rect.height / 2` is) put the glyph box's *top* at the centre,
        // so a 14 px label in a 24 px tab spanned 12..26 and crossed the tab's bottom edge.
        let has_icon = tab.has_icon();
        let text_x = tab_rect.x
            + TAB_TEXT_INSET as i32
            + if has_icon { (TAB_ICON_SIZE + TAB_ICON_GAP) as i32 } else { 0 };
        // The **effective font** — the resolved theme/caller font — replacing the fixed Arial face,
        // so the tab title honours the theme body font and the user's text scale (D09-STYLE-01).
        // The line height is measured once per frame and passed in, rather than measured per tab.
        let text_y = tab_rect.y + (tab_rect.height as i32 - title_height) / 2;
        // The tab's icon, from whichever form is active. The legacy `Image` used to reserve the slot
        // above and then never be drawn -- the tab was wider for an icon that did not exist. Both
        // forms now paint through the shared seams.
        let icon_rect = has_icon.then(|| {
            let side = tab_rect.height.clamp(1, TAB_ICON_SIZE);
            Rect::new(
                tab_rect.x + TAB_TEXT_INSET as i32,
                tab_rect.y + (tab_rect.height as i32 - side as i32) / 2,
                side,
                side,
            )
        });
        if let (Some(source), Some(icon_rect)) = (tab.icon_source, icon_rect) {
            if let Err(error) =
                crate::widget::paint_icon(context, source.into(), icon_rect, text_color)
            {
                log::warn!("tab bar tab {index} icon failed: {error:?}");
            }
        }
        #[cfg(feature = "image")]
        if tab.icon_source.is_none() {
            if let (Some(image), Some(icon_rect)) = (tab.icon.as_ref(), icon_rect) {
                if !crate::widget::draw_icon_image(context, image, icon_rect) {
                    log::warn!("tab bar tab {index} icon image could not be drawn");
                }
            }
        }
        // A horizontal title wider than the tab it sits in is elided with the shared primitive
        // rather than painted across the neighbouring tab. The elided string was decided once, in
        // `draw`'s cache refresh, so this is a borrow rather than a search per frame (K-P09); a
        // fitting title is the tab's own string.
        context.draw_text(
            Point::new(text_x, text_y),
            title,
            font,
            text_color,
            HorizontalAlignment::Left,
        );

        // Draw close button if closable.
        if let Some(close_rect) = self.close_rect(index) {
            // Secondary chrome: a tint of the resolved foreground, muted further when
            // the tab is disabled.
            let close_color = if !is_enabled {
                text_color.blend(&disabled_tab, 0.4)
            } else {
                text_color.blend(&current_tab, 0.4)
            };
            context.draw_line(
                Point::new(close_rect.x, close_rect.y),
                Point::new(
                    close_rect.x + close_rect.width as i32,
                    close_rect.y + close_rect.height as i32,
                ),
                close_color,
            );
            context.draw_line(
                Point::new(close_rect.x + close_rect.width as i32, close_rect.y),
                Point::new(close_rect.x, close_rect.y + close_rect.height as i32),
                close_color,
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Widget trait
// ---------------------------------------------------------------------------
impl Widget for TabBar {
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
        crate::core::Size::new(400, 30)
    }
    impl_draw_bridge!();
    impl_widget_property_hooks!();

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
    /// Resolves a published event name to the control's own signal.
    ///
    /// # Why the field is named per arm
    ///
    /// The published name and the Rust field name are not always the same (`find_next` is backed by
    /// `find_next_signal`, `dismissed` by a `Signal1<()>` field). `connect_event` validates a name
    /// against the capability table and registers a hub slot; only `event_signal_dyn` joins that
    /// name to the signal the control actually **emits**. A wrong arm is worse than no arm, because
    /// it reports a wire as live and never fires it, so each field is named explicitly here rather
    /// than derived from the published name.
    fn event_signal_dyn(&self, name: &str) -> Option<crate::signal::EventSignalRef> {
        use crate::signal::EventSignalRef;
        match name {
            "current_changed" => {
                Some(EventSignalRef::mapped("current_changed", &self.current_changed, |v| {
                    CapabilityValue::UInt(*v as u64)
                }))
            }
            "tab_close_requested" => Some(EventSignalRef::mapped(
                "tab_close_requested",
                &self.tab_close_requested,
                |v| CapabilityValue::UInt(*v as u64),
            )),
            "tab_moved" => Some(EventSignalRef::mapped("tab_moved", &self.tab_moved, |v| {
                // The declared shape is `Tuple2` of `UInt`, so the payload arrives as the
                // (old_index, new_index) pair rather than its `Debug` spelling.
                CapabilityValue::Tuple(crate::compat::Vec::from([
                    CapabilityValue::UInt(v.0 as u64),
                    CapabilityValue::UInt(v.1 as u64),
                ]))
            })),
            "tab_bar_clicked" => Some(EventSignalRef::mapped(
                "tab_bar_clicked",
                &self.tab_bar_clicked,
                |v| match v {
                    Some(value) => crate::widget::capability::CapabilityValue::UInt(*value as u64),
                    None => crate::widget::capability::CapabilityValue::Null,
                },
            )),
            _ => None,
        }
    }
}

/// `TabBar`'s property contract.
///
/// `tab_count` is derived from the tab vector and `current_index` is optional,
/// so the read path publishes `Null` for "no tab selected" while the write path
/// narrows through `expect_usize` before storing a `usize`.
impl WidgetProperties for TabBar {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            // The whole tab strip as records (BLUE26 B26.09): `add_tab` / `remove_tab` take
            // arguments the bare command channel cannot carry, so the list itself is the property
            // and `set_tabs` is its published command form.
            "tabs" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &TAB_BAR_TAB_SCHEMA,
                    self.tabs
                        .iter()
                        .map(tab_bar_tab_record)
                        .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
                )))
            }
            "tab_count" => Ok(CapabilityValue::UInt(self.tab_count() as u64)),
            "current_index" => match self.current_index() {
                Some(idx) => Ok(CapabilityValue::UInt(idx as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "closable" => Ok(CapabilityValue::Bool(self.closable())),
            "movable" => Ok(CapabilityValue::Bool(self.movable())),
            "tab_min_width" => Ok(CapabilityValue::UInt(self.tab_min_width() as u64)),
            "tab_max_width" => Ok(CapabilityValue::UInt(self.tab_max_width() as u64)),
            "direction" => {
                Ok(CapabilityValue::String(text_direction_to_str(self.direction()).to_string()))
            }
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "current_index" => match value {
                // `Null` mirrors what `get` publishes when no tab is selected, so
                // the two directions agree on the meaning of an absent index.
                CapabilityValue::Null => {
                    self.clear_current_index();
                    Ok(())
                }
                other => {
                    self.set_current_index(expect_usize(other)?);
                    Ok(())
                }
            },
            "closable" => {
                self.set_closable(expect_bool(value)?);
                Ok(())
            }
            "movable" => {
                self.set_movable(expect_bool(value)?);
                Ok(())
            }
            "tab_min_width" => {
                self.set_tab_min_width(expect_u32_saturating(value)?);
                Ok(())
            }
            "tab_max_width" => {
                self.set_tab_max_width(expect_u32_saturating(value)?);
                Ok(())
            }
            "direction" => {
                self.set_direction(expect_text_direction(value)?);
                Ok(())
            }
            // A whole-strip replacement. Checked in full before anything is replaced, then rebuilt
            // through the control's own `clear` + `add_tab`, so the current-index reset is the one
            // `clear` owns rather than a second spelling of it.
            "tabs" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                list.validate_against(&TAB_BAR_TAB_SCHEMA).map_err(|violation| {
                    log::warn!("tab_bar.tabs refused: {violation}");
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("tabs"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let mut tabs = Vec::with_capacity(list.len());
                for record in list.items() {
                    tabs.push(tab_bar_tab_from_record(record)?);
                }
                self.clear();
                for tab in tabs {
                    let index = self.add_tab(tab.title().to_string());
                    if let Some(target) = self.tabs.get_mut(index) {
                        target.set_tooltip(tab.tooltip().to_string());
                        target.set_enabled(tab.is_enabled());
                        target.set_icon_source(tab.icon_source());
                    }
                }
                self.base.request_redraw();
                Ok(())
            }
            "tab_count" => Err(CapabilityAccessError::ReadOnlyProperty),
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "tabs",
            "tab_count",
            "current_index",
            "closable",
            "movable",
            "tab_min_width",
            "tab_max_width",
            "direction",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `tab_bar` publishes.
    ///
    /// `add_tab` takes the new tab's title and `remove_tab` takes the index to
    /// remove, so neither can complete without a payload: they are refused as
    /// [`CapabilityAccessError::OutOfRange`], meaning the name is valid and the
    /// argument is what is missing. `set_tabs` is the published whole-strip form:
    /// its rows travel through the `tabs` property (B26.03).
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "add_tab" | "remove_tab" | "set_tabs" => Err(CapabilityAccessError::OutOfRange),
            // Any other `set_foo` name carries its value through the property route,
            // so the shared default reports that a payload is needed rather than
            // claiming the control has never heard of it.
            _ if name.starts_with("set_") => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

// ---------------------------------------------------------------------------
// EventHandler trait
// ---------------------------------------------------------------------------
impl EventHandler for TabBar {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }
        if self.auto_hide && self.tabs.len() <= 1 {
            return;
        }
        match event {
            Event::MouseMove { pos } => {
                let prev = self.hovered_index;
                self.hovered_index = self.tab_at_position(*pos);
                if prev != self.hovered_index {
                    // A different tab is a different hint; re-register before the redraw.
                    self.sync_hover_tooltip();
                    self.base.request_redraw();
                }
                // The bubble follows the pointer; this is the manager's own follow mode.
                self.tooltips.note_pointer(*pos);
                // Live reorder preview: the drop target follows the pointer, so the
                // insertion line is where the release would land.
                if self.dragging_from.is_some() {
                    let target = self.tab_at_position(*pos);
                    if target != self.drag_target {
                        self.drag_target = target;
                        self.base.request_redraw();
                    }
                }
            }
            Event::MousePress { pos, button, .. } if *button == 1 => {
                // The scroll buttons sit over the strip's ends and take precedence.
                if let Some((left, right)) = self.scroll_button_rects() {
                    if left.contains_point(*pos) {
                        self.scroll_by(-1);
                        return;
                    }
                    if right.contains_point(*pos) {
                        self.scroll_by(1);
                        return;
                    }
                }
                self.tab_bar_clicked.emit(self.tab_at_position(*pos));
                // Check close button first.
                if self.closable {
                    for i in 0..self.tabs.len() {
                        if let Some(close_hit_area) = self.close_hit_area(i) {
                            if close_hit_area.contains(*pos) && self.tabs[i].enabled {
                                self.tab_close_requested.emit(i);
                                return;
                            }
                        }
                    }
                }
                // Otherwise, select tab.
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        self.set_current_index(index);
                        // A press on a movable tab arms a drag; the release decides
                        // whether it moved. Arming on press rather than on the first
                        // move means a click that happens to jitter by a pixel still
                        // behaves as a click.
                        if self.movable {
                            self.dragging_from = Some(index);
                        }
                    }
                }
            }
            Event::MouseRelease { pos, .. } => {
                // Completing the move on release, and only on release, is what makes
                // `movable` mean "drag to reorder" rather than "reorder on click".
                if let Some(from) = self.dragging_from.take() {
                    let target = self.tab_at_position(*pos);
                    self.drag_target = None;
                    if let Some(to) = target {
                        self.move_tab(from, to);
                    }
                    self.base.request_redraw();
                }
            }
            // A double click on the strip's empty space asks the host for a new tab; a
            // double click on a tab or on the scroll chrome is not that gesture (ADV-009).
            Event::MouseDoubleClick { pos, button } if *button == 1 => {
                if self.geometry().contains_point(*pos) && self.tab_at_position(*pos).is_none() {
                    let on_scroll_button =
                        self.scroll_button_rects().is_some_and(|(left, right)| {
                            left.contains_point(*pos) || right.contains_point(*pos)
                        });
                    if !on_scroll_button {
                        self.new_tab_requested.emit();
                    }
                }
            }
            // A wheel over the strip walks the visible run, which is what `scroll_by` was
            // written for and what the event path never reached (ADV-13). One notch is one tab;
            // the event's positive y means "towards the end of the run".
            Event::Wheel { delta, modifiers } => {
                let scroll = WheelScroll::from_event(*delta, *modifiers, 1);
                if scroll.lines_y != 0 {
                    self.scroll_by(scroll.lines_y);
                }
            }
            // The middle button closes the tab under the pointer, the browser convention. It
            // goes through the same request the close button emits, so a host that already
            // handles `tab_close_requested` (and decides how few tabs it will keep) needs no
            // second path.
            Event::MousePress { pos, button: 2, .. } if self.closable => {
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        self.tab_close_requested.emit(index);
                    }
                }
            }
            Event::MouseLeave { .. } => {
                // The hint belongs to the hover it was registered for; leaving drops it.
                self.hovered_index = None;
                self.tooltip_source = None;
                self.tooltips.note_leave();
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                crate::event::key_codes::LEFT => {
                    self.step_selection(-1);
                }
                crate::event::key_codes::RIGHT => {
                    self.step_selection(1);
                }
                crate::event::key_codes::HOME => {
                    if !self.tabs.is_empty() {
                        self.set_current_index(0);
                    }
                }
                crate::event::key_codes::END => {
                    if !self.tabs.is_empty() {
                        self.set_current_index(self.tabs.len() - 1);
                    }
                }
                crate::event::key_codes::DELETE if self.closable => {
                    if let Some(index) = self.current_index {
                        self.tab_close_requested.emit(index);
                    }
                }
                _ => {}
            },
            #[cfg(feature = "touch")]
            Event::Tap { pos } => {
                // Check close button first.
                if self.closable {
                    for i in 0..self.tabs.len() {
                        if let Some(close_hit_area) = self.close_hit_area(i) {
                            if close_hit_area.contains(*pos) && self.tabs[i].enabled {
                                self.tab_close_requested.emit(i);
                                return;
                            }
                        }
                    }
                }
                // Otherwise, select tab.
                if let Some(index) = self.tab_at_position(*pos) {
                    if self.tabs[index].enabled {
                        self.set_current_index(index);
                    }
                }
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

// ---------------------------------------------------------------------------
// Draw trait
// ---------------------------------------------------------------------------
impl Draw for TabBar {
    fn draw(&mut self, context: &mut RenderContext) {
        if self.auto_hide && self.tabs.len() <= 1 {
            return;
        }
        // Measure every label once, up front, for the whole strip — but only when a title or the
        // font actually changed (K-P09). A tab's width depends on its own label, so measuring
        // while drawing would leave tab `n` laid out from the estimate and tab `n+1` from a real
        // metric; a strip whose later tabs are a different size from its first. The cache keeps
        // that one-measurement rule without paying it on static frames.
        let font = self.label_font();
        self.refresh_measured_title_widths(&font, context);
        // The elided forms are refreshed in their own pass, because a mutable borrow of the cache
        // cannot coexist with the painter's immutable one.
        for i in 0..self.tabs.len() {
            if let Some(tab_rect) = self.tab_rect(i) {
                self.refresh_elided_title(i, tab_rect, &font, &mut |text: &str| {
                    context.measure_text(text, &font).width
                });
            }
        }
        // One line-height measurement for the whole strip instead of one per tab.
        let title_height = context.measure_text("M", &font).height as i32;
        let chrome = self.tab_chrome();
        for i in 0..self.tabs.len() {
            if let Some(tr) = self.tab_rect(i) {
                // Both are immutable borrows, so the title stays borrowed and a static strip
                // allocates nothing here (K-P09).
                let title = self.painted_title(i);
                self.draw_tab(context, i, tr, &chrome, &font, title, title_height);
            }
        }

        // ── Live drag preview ──
        if let (Some(from), Some(target)) = (self.dragging_from, self.drag_target) {
            if let Some(rect) = self.tab_rect(target) {
                let x = if target > from { rect.x + rect.width as i32 } else { rect.x };
                context.draw_line(
                    Point::new(x, rect.y),
                    Point::new(x, rect.y + rect.height as i32),
                    chrome.current_tab.contrast_color(),
                );
            }
        }

        // ── Overflow scroll buttons ──
        if let Some((left, right)) = self.scroll_button_rects() {
            let ink = chrome.text_color;
            let side = left.width.min(left.height).min(12);
            crate::widget::draw_icon_centered(context, left, side, ink, IconName::ChevronLeft);
            crate::widget::draw_icon_centered(context, right, side, ink, IconName::ChevronRight);
        }

        // ── Keyboard focus ring ──
        //
        // Drawn after the tabs so no tab fill can paint over it, and gated on the reason
        // focus arrived: a pointer press focuses the strip without painting a ring under
        // the cursor, while Tab and Shortcut draw one. The ring follows the current tab
        // rather than the whole strip, so it frames the part the keyboard is actually on.
        if self.base.draws_focus_ring() {
            let ring = FocusRing::for_control(self.focus_ring_rect(), self.focus_ring_corner());
            if ring.is_drawable() {
                // The fallback is the current tab's own contrasting tone; an active theme
                // overrides it with its declared `outline` token.
                context.draw_rounded_rect_stroke(
                    ring.rect,
                    ring.radius,
                    focus_ring_color(chrome.current_tab.contrast_color()),
                    FOCUS_RING_WIDTH,
                );
            }
        }

        // The tooltip pipeline owns the bubble; keeping it in the same control means a host does
        // not need a second mounted widget to read a truncated tab title (ADV-11).
        let geometry = self.geometry();
        self.tooltips.set_geometry(geometry);
        self.tooltips.draw(context);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Rect;

    #[test]
    fn tabbar_creation_defaults() {
        let tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(tb.is_empty());
        assert_eq!(tb.count(), 0);
        assert_eq!(tb.current_index(), None);
        assert!(!tb.closable());
        assert!(!tb.movable());
        assert_eq!(tb.tab_min_width(), 40);
        assert_eq!(tb.tab_max_width(), 200);
    }

    #[test]
    fn tabbar_add_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        let idx = tb.add_tab("Tab 1".to_string());
        assert_eq!(idx, 0);
        assert_eq!(tb.count(), 1);
        assert_eq!(tb.current_index(), Some(0));
        assert_eq!(tb.tab_text(0), Some("Tab 1"));
    }

    #[test]
    fn tabbar_typed_icon_stays_attached_to_tab_through_reordering() {
        static RING: crate::widget::IconData = crate::widget::IconData {
            name: "tab-ring",
            grid: 24,
            paths: &["M2-2H22V-22H2Z M7-7V-17H17V-7Z"],
        };

        let mut tabs = TabBar::new(Rect::new(0, 0, 400, 32));
        tabs.add_tab("Settings".into());
        tabs.set_tab_icon_source(0, Some(IconSource::static_geometry(&RING)))
            .expect("existing tab accepts typed source");
        tabs.insert_tab(0, "Home".into());
        assert_eq!(
            tabs.set_tab_icon_source(2, None),
            Err(IconItemError::IndexOutOfBounds { index: 2, len: 2 })
        );
        assert!(crate::widget::svg::render_to_svg(&mut tabs).contains("fill-rule=\"nonzero\""));
        tabs.remove_tab(1);
        assert_eq!(
            tabs.set_tab_icon_source(1, None),
            Err(IconItemError::IndexOutOfBounds { index: 1, len: 1 })
        );
    }

    /// A tab's legacy `Image` reserved the icon slot in the width measurement and was then never
    /// drawn, so the tab was wider for an icon that did not exist. It is now reachable through
    /// `tab_mut` and painted through the shared seam.
    #[cfg(feature = "image")]
    #[test]
    fn tabbar_legacy_tab_image_is_drawn_and_cleared() {
        let mut tabs = TabBar::new(Rect::new(0, 0, 400, 32));
        tabs.add_tab("Settings".into());
        let render = |tabs: &mut TabBar| crate::widget::svg::render_to_svg(tabs);
        let plain = render(&mut tabs);
        assert!(!plain.contains("<image"), "a tab with no icon draws no image");

        tabs.tab_mut(0).expect("the tab exists").set_icon(Some(
            Image::try_from_rgba(vec![200; 4 * 4 * 4], 4, 4)
                .expect("the buffer matches its declared size"),
        ));
        assert!(tabs.tab(0).expect("the tab exists").has_icon());
        assert!(render(&mut tabs).contains("<image"), "the image must reach the backend");

        // An explicit typed clear clears the legacy image too.
        tabs.set_tab_icon_source(0, None).expect("the tab exists");
        assert!(!tabs.tab(0).expect("the tab exists").has_icon());
        assert!(!render(&mut tabs).contains("<image"), "clear must release the image slot");
    }

    #[test]
    fn tabbar_multiple_tabs() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("First".to_string());
        tb.add_tab("Second".to_string());
        tb.add_tab("Third".to_string());
        assert_eq!(tb.count(), 3);
        assert_eq!(tb.tab_text(0), Some("First"));
        assert_eq!(tb.tab_text(1), Some("Second"));
        assert_eq!(tb.tab_text(2), Some("Third"));
    }

    #[test]
    fn tabbar_insert_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("C".to_string());
        tb.insert_tab(1, "B".to_string());
        assert_eq!(tb.count(), 3);
        assert_eq!(tb.tab_text(1), Some("B"));
    }

    #[test]
    fn tabbar_remove_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());
        tb.add_tab("C".to_string());
        tb.remove_tab(1);
        assert_eq!(tb.count(), 2);
        assert_eq!(tb.tab_text(1), Some("C"));
    }

    #[test]
    fn tabbar_clear() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());
        tb.clear();
        assert!(tb.is_empty());
        assert_eq!(tb.current_index(), None);
    }

    #[test]
    fn tabbar_set_current_index() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());
        tb.set_current_index(1);
        assert_eq!(tb.current_index(), Some(1));
        tb.set_current_index(0);
        assert_eq!(tb.current_index(), Some(0));
    }

    #[test]
    fn tabbar_set_current_index_requests_redraw() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("A".to_string());
        tb.add_tab("B".to_string());

        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        tb.base.redraw_requested.connect({
            let flag = std::sync::Arc::clone(&fired);
            move || flag.store(true, std::sync::atomic::Ordering::SeqCst)
        });

        tb.set_current_index(1);

        assert!(fired.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn tabbar_set_tab_text() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("Old".to_string());
        tb.set_tab_text(0, "New".to_string());
        assert_eq!(tb.tab_text(0), Some("New"));
    }

    #[test]
    fn tabbar_tab_enabled() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("Tab".to_string());
        assert_eq!(tb.tab_enabled(0), Some(true));
        tb.set_tab_enabled(0, false);
        assert_eq!(tb.tab_enabled(0), Some(false));
    }

    #[test]
    fn tabbar_closable_movable() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(!tb.closable());
        tb.set_closable(true);
        assert!(tb.closable());
        assert!(!tb.movable());
        tb.set_movable(true);
        assert!(tb.movable());
    }

    #[test]
    fn tabbar_tab_position_shape() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert_eq!(tb.tab_position(), TabPosition::North);
        tb.set_tab_position(TabPosition::South);
        assert_eq!(tb.tab_position(), TabPosition::South);
        assert_eq!(tb.tab_shape(), TabShape::Rounded);
        tb.set_tab_shape(TabShape::Rectangular);
        assert_eq!(tb.tab_shape(), TabShape::Rectangular);
    }

    #[test]
    fn tabbar_min_max_width() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.set_tab_min_width(50);
        assert_eq!(tb.tab_min_width(), 50);
        tb.set_tab_max_width(100);
        assert_eq!(tb.tab_max_width(), 100);
    }

    #[test]
    fn tabbar_signal_accessors() {
        let tb = TabBar::new(Rect::new(0, 0, 400, 24));
        let _ = &tb.current_changed;
        let _ = &tb.tab_close_requested;
        let _ = &tb.tab_moved;
    }

    #[test]
    fn tabbar_geometry_delegation() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.set_geometry(Rect::new(10, 10, 500, 30));
        assert_eq!(tb.geometry(), Rect::new(10, 10, 500, 30));
    }

    #[test]
    fn tabbar_visibility() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(tb.is_visible());
        tb.hide();
        assert!(!tb.is_visible());
        tb.show();
        assert!(tb.is_visible());
    }

    #[test]
    fn tabbar_id_kind() {
        let a = TabBar::new(Rect::new(0, 0, 400, 24));
        let b = TabBar::new(Rect::new(0, 0, 400, 24));
        assert_ne!(a.id(), b.id());
        assert_eq!(a.kind(), WidgetKind::TabBar);
    }

    #[test]
    fn tabbar_draw_produces_svg() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("One".to_string());
        tb.add_tab("Two".to_string());
        tb.set_current_index(0);
        let svg = crate::widget::svg::render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"));
        assert!(svg.len() > 100);
    }

    /// Regression: `set_tab_min_width` used to leave `tab_max_width` behind, so a minimum above
    /// the default maximum produced an inverted pair and `compute_tab_width`'s `clamp(min, max)`
    /// **panicked**. Both setters must keep `min <= max`.
    #[test]
    fn tabbar_min_width_above_max_does_not_invert_pair() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        assert!(tb.tab_min_width() <= tb.tab_max_width());

        tb.set_tab_min_width(5000);
        assert_eq!(tb.tab_min_width(), 5000);
        assert!(
            tb.tab_min_width() <= tb.tab_max_width(),
            "min {} must not exceed max {}",
            tb.tab_min_width(),
            tb.tab_max_width()
        );
        assert_eq!(tb.tab_max_width(), 5000, "maximum must rise to meet the new minimum");

        // The clamp itself must not panic, and neither must a full render.
        tb.add_tab("A very long tab title that would exceed the default maximum".to_string());
        assert!(tb.tab_rect(0).is_some());
        let svg = crate::widget::svg::render_to_svg(&mut tb);
        assert!(svg.starts_with("<svg"));
    }

    /// The mirror case: an inverted *maximum* must be pulled back up to the minimum, not left to
    /// poison the clamp.
    #[test]
    fn tabbar_max_width_below_min_does_not_invert_pair() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.set_tab_min_width(120);
        tb.set_tab_max_width(10);
        assert!(tb.tab_min_width() <= tb.tab_max_width());
        assert_eq!(tb.tab_max_width(), 120);
        tb.add_tab("X".to_string());
        assert!(tb.tab_rect(0).is_some());
    }

    #[test]
    fn tabbar_mouse_click_selects_tab() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("First".to_string());
        tb.add_tab("Second".to_string());
        // Click on the first tab area
        tb.handle_event(&Event::MousePress { pos: Point::new(5, 5), button: 1, modifiers: 0 });
        assert_eq!(tb.current_index(), Some(0));
        // Hover sets hovered_index
        tb.handle_event(&Event::MouseMove { pos: Point::new(5, 5) });
    }

    /// The defect: the tab position was `(tab_width + spacing) * index` with no reference to
    /// `rect.width`, so tabs past the strip's end were drawn entirely outside the control.
    ///
    /// The expectation is read off the control's own API — every tab's rect must be inside the
    /// control's own rectangle — rather than restating any particular fitted width.
    #[test]
    fn tabbar_every_tab_rect_stays_inside_the_control() {
        let geometry = Rect::new(0, 0, 240, 120);
        for count in [1usize, 2, 3, 6, 12] {
            let mut tb = TabBar::new(geometry);
            for i in 0..count {
                tb.add_tab(format!("Tab {i}"));
            }
            let mut right_edge = 0i32;
            for i in 0..count {
                let r = tb.tab_rect(i).expect("every added tab has a rect");
                assert!(
                    r.x >= geometry.x && r.right() <= geometry.right(),
                    "tab {i} of {count} at {r:?} escapes {geometry:?}"
                );
                assert!(r.width > 0, "tab {i} of {count} was shrunk to nothing");
                right_edge = right_edge.max(r.right());
            }
            assert!(
                right_edge <= geometry.right(),
                "{count} tabs reach x = {right_edge} in a {}px strip",
                geometry.width
            );
        }
    }

    /// Shrinking is the strip's overflow rule, and a strip that fits must **not** be shrunk: a
    /// fit rule applied unconditionally would make two tabs on a wide bar tiny.
    #[test]
    fn tabbar_tabs_are_not_shrunk_while_they_fit() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("One".to_string());
        tb.add_tab("Two".to_string());
        let fitted = tb.tab_rect(0).unwrap().width;
        assert_eq!(tb.tab_rect(1).unwrap().width, fitted, "fitted tabs share one width");
        assert!(fitted >= tb.tab_min_width());
        assert!(fitted * 2 < tb.geometry().width, "two tabs must not fill a 400px strip");
    }

    /// In a right-to-left strip the first tab belongs at the strip's **right** edge, and the strip
    /// fills leftward.
    ///
    /// # The defect this pins
    ///
    /// A tab strip is a sequence, so "where does tab 0 sit" is a statement about reading order. The
    /// control placed tab 0 at `rect.x` unconditionally, which in an Arabic or Hebrew interface put
    /// the first tab where the reader looks last and made the visual order contradict the order of
    /// the pages the tabs select.
    #[test]
    fn a_right_to_left_strip_starts_at_its_right_edge() {
        let geometry = Rect::new(0, 0, 240, 120);
        for count in [1usize, 2, 3, 6] {
            let mut tb = TabBar::new(geometry);
            for i in 0..count {
                tb.add_tab(format!("Tab {i}"));
            }
            tb.set_direction(crate::core::TextDirection::RightToLeft);

            let first = tb.tab_rect(0).expect("every added tab has a rect");
            assert_eq!(
                first.right(),
                geometry.right(),
                "with {count} tabs, tab 0 must begin at the strip's beginning (its right edge)"
            );

            // The order is reversed on screen: every later tab is strictly further left.
            let mut previous_x = first.x;
            for i in 1..count {
                let r = tb.tab_rect(i).expect("every added tab has a rect");
                assert!(
                    r.right() <= previous_x,
                    "tab {i} of {count} must sit left of tab {} ({} > {})",
                    i - 1,
                    r.right(),
                    previous_x
                );
                previous_x = r.x;
            }
            // And the run as a whole stays inside the control on the left as well.
            let last = tb.tab_rect(count - 1).unwrap();
            assert!(
                last.x >= geometry.x,
                "the last tab of {count} starts at {} and escapes the strip",
                last.x
            );
        }
    }

    /// The mirror of the overflow rule: an RTL strip whose tabs no longer fit must shrink them
    /// rather than let the run grow off the left end.
    ///
    /// This is the case where reversing the *index* instead of the *offset* would have looked
    /// correct while the tabs still fit and broken as soon as they did not, so the test drives the
    /// shrinking path deliberately.
    #[test]
    fn a_right_to_left_strip_keeps_every_tab_inside_when_they_overflow() {
        let geometry = Rect::new(0, 0, 120, 120);
        let mut tb = TabBar::new(geometry);
        for i in 0..8 {
            tb.add_tab(format!("Tab {i}"));
        }
        tb.set_direction(crate::core::TextDirection::RightToLeft);
        for i in 0..8 {
            let r = tb.tab_rect(i).expect("every added tab has a rect");
            assert!(
                r.x >= geometry.x && r.right() <= geometry.right(),
                "tab {i} at {r:?} escapes the {geometry:?} strip"
            );
            assert!(r.width > 0, "tab {i} was shrunk to nothing");
        }
    }

    /// A left-to-right strip must be byte-for-byte what it was before the field existed, and the
    /// two directions must genuinely differ — otherwise "defaults to LTR" could be satisfied by
    /// ignoring the setting entirely.
    #[test]
    fn setting_the_strip_direction_moves_the_tabs_and_defaults_unchanged() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        fn render(direction: Option<crate::core::TextDirection>) -> String {
            let mut tb = TabBar::new(Rect::new(0, 0, 300, 120));
            tb.add_tab("One".to_string());
            tb.add_tab("Two".to_string());
            tb.add_tab("Three".to_string());
            if let Some(d) = direction {
                tb.set_direction(d);
            }
            crate::widget::svg::render_to_svg(&mut tb)
        }

        let untouched = render(None);
        let ltr = render(Some(crate::core::TextDirection::LeftToRight));
        let rtl = render(Some(crate::core::TextDirection::RightToLeft));
        assert_eq!(untouched, ltr, "the default must be left-to-right, so the output is identical");
        assert_ne!(ltr, rtl, "a right-to-left strip must draw its tabs somewhere else");
    }

    /// The vertical strips are attached to a side, not to a line, so the direction must not mirror
    /// them onto each other.
    ///
    /// # Why this is an assertion and not a comment
    ///
    /// `West` and `East` are two of the four `TabPosition` values the same field could plausibly be
    /// applied to. "Which side is the strip on" is a layout choice a caller made; applying the
    /// writing direction to it would silently move a strip the caller had explicitly placed.
    #[test]
    fn a_vertical_strip_is_not_mirrored_by_the_direction() {
        for position in [TabPosition::West, TabPosition::East] {
            let mut ltr = TabBar::new(Rect::new(0, 0, 240, 120));
            let mut rtl = TabBar::new(Rect::new(0, 0, 240, 120));
            for tb in [&mut ltr, &mut rtl] {
                tb.add_tab("One".to_string());
                tb.add_tab("Two".to_string());
                tb.set_tab_position(position);
            }
            rtl.set_direction(crate::core::TextDirection::RightToLeft);
            for i in 0..2 {
                assert_eq!(
                    ltr.tab_rect(i),
                    rtl.tab_rect(i),
                    "{position:?} tab {i} moved when only the direction changed"
                );
            }
        }
    }

    /// The direction is reachable from the property API, and the token written is the token read
    /// back.
    ///
    /// A settable-only field, or one whose `get` and `set` spellings disagree, would leave a caller
    /// driving the control from a document unable to restore what they just set.
    #[test]
    fn the_direction_round_trips_through_the_property_api() {
        let mut tb = TabBar::new(Rect::new(0, 0, 240, 24));
        tb.add_tab("One".to_string());

        assert_eq!(tb.get("direction").unwrap().as_str(), Some("ltr"), "default is ltr");
        tb.set("direction", CapabilityValue::String(String::from("rtl"))).unwrap();
        assert_eq!(tb.direction(), crate::core::TextDirection::RightToLeft);
        assert_eq!(tb.get("direction").unwrap().as_str(), Some("rtl"));
        // The long spelling is accepted too, so a caller need not know which one this crate picked.
        tb.set("direction", CapabilityValue::String(String::from("left_to_right"))).unwrap();
        assert_eq!(tb.direction(), crate::core::TextDirection::LeftToRight);
    }

    /// The three `TabShape` values were drawn identically, so the property changed nothing on
    /// screen. Each shape must now produce a different drawing.
    #[test]
    fn tabbar_tab_shapes_are_distinct() {
        // Holds the crate-wide theme guard: this test renders, and a concurrent
        // test that switches the appearance would otherwise change a later frame.
        let _theme_guard = crate::style::theme_test_guard();
        fn render(shape: TabShape) -> String {
            let mut tb = TabBar::new(Rect::new(0, 0, 240, 120));
            tb.add_tab("One".to_string());
            tb.add_tab("Two".to_string());
            tb.set_current_index(0);
            tb.set_tab_shape(shape);
            crate::widget::svg::render_to_svg(&mut tb)
        }

        let rounded = render(TabShape::Rounded);
        let triangular = render(TabShape::Triangular);
        let rectangular = render(TabShape::Rectangular);
        assert_ne!(rounded, triangular, "rounded and triangular must not draw the same");
        assert_ne!(rounded, rectangular, "rounded and rectangular must not draw the same");
        assert_ne!(triangular, rectangular, "triangular and rectangular must not draw the same");
        // Each shape leaves its own mark: rounded corners, a closed triangle path, plain rects.
        assert!(rounded.contains("rx="), "a rounded tab must have rounded corners");
        assert!(
            triangular.contains("<polygon") || triangular.contains("<path"),
            "a triangular tab must be drawn as a triangle"
        );
        assert!(
            !rectangular.contains("rx=") && !rectangular.contains("<polygon"),
            "a rectangular tab must be a plain rectangle"
        );
    }

    /// `set_tab_text` asks for a frame.
    ///
    /// The title is what the strip measures, elides and draws, so a rename that did not mark
    /// damage left the old caption on screen.
    #[test]
    fn setting_tab_text_requests_a_redraw() {
        let mut tb = TabBar::new(Rect::new(0, 0, 300, 32));
        tb.add_tab("First".to_string());
        let fired = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        {
            let f = std::sync::Arc::clone(&fired);
            let scope = tb.connection_scope();
            tb.redraw_requested_signal().connect_scoped(scope, move || {
                f.store(true, std::sync::atomic::Ordering::SeqCst);
            });
        }

        tb.set_tab_text(0, "Renamed".to_string());
        assert!(fired.load(std::sync::atomic::Ordering::SeqCst), "a rename must be drawn");

        fired.store(false, std::sync::atomic::Ordering::SeqCst);
        tb.set_tab_text(9, "Nowhere".to_string());
        assert!(
            !fired.load(std::sync::atomic::Ordering::SeqCst),
            "an index that addresses nothing must do nothing"
        );
    }

    /// A multi-byte title is elided on character boundaries, never inside a CJK glyph.
    ///
    /// The strip shrinks every tab to `available / count`, so a title can be wider than
    /// the tab it is painted in; the shared primitive keeps the retained prefix on whole
    /// characters and marks the cut with the crate's single ellipsis.
    #[test]
    fn a_wide_cjk_tab_title_is_elided_to_its_label_band() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("\u{4e16}\u{754c}\u{4f60}\u{597d}\u{4e16}\u{754c}\u{4f60}\u{597d}".to_string());
        // 7 px per character, so the 40 px tab (28 px label band) keeps three characters
        // plus the ellipsis; a byte budget would have cut this string mid-glyph.
        let mut measure = |text: &str| text.chars().count() as u32 * 7;
        assert_eq!(
            tb.fitted_title(0, Rect::new(0, 0, 40, 24), &mut measure),
            "\u{4e16}\u{754c}\u{4f60}\u{2026}"
        );
        tb.set_tab_text(0, "\u{1f389}\u{1f389}\u{1f389}\u{1f389}\u{1f389}".to_string());
        assert_eq!(
            tb.fitted_title(0, Rect::new(0, 0, 40, 24), &mut measure),
            "\u{1f389}\u{1f389}\u{1f389}\u{2026}",
            "an emoji is a single character to the budget, not four bytes"
        );
    }

    /// A title that fits its tab is borrowed, so the paint path keeps the exact call and
    /// allocates nothing for it.
    #[test]
    fn a_fitting_tab_title_is_borrowed() {
        let mut tb = TabBar::new(Rect::new(0, 0, 400, 24));
        tb.add_tab("One".to_string());
        let mut measure = |text: &str| text.chars().count() as u32 * 7;
        assert!(matches!(
            tb.fitted_title(0, Rect::new(0, 0, 40, 24), &mut measure),
            crate::compat::Cow::Borrowed("One")
        ));
    }

    /// Keyboard focus paints the ring on the current tab; a pointer press does not.
    ///
    /// The strip reported `WidgetState::Focused` for keyboard focus but had no draw site
    /// for the ring, so the state had no visible consumer. The ring is the only 2 px
    /// stroke the strip emits, which makes its presence observable in the render.
    #[test]
    fn a_keyboard_focused_strip_paints_the_ring_and_pointer_focus_does_not() {
        // The ring's colour reads the process-wide theme; hold the guard so a concurrent
        // appearance switch cannot change this render mid-assertion.
        let _guard = crate::style::theme_test_guard();
        let make = || {
            let mut bar = TabBar::new(Rect::new(0, 0, 200, 32));
            bar.add_tab("One".to_string());
            bar.set_current_index(0);
            bar
        };

        let mut keyboard = make();
        keyboard.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Tab });
        assert!(keyboard.base.draws_focus_ring(), "Tab focus must ask for a ring");
        let focused = crate::widget::svg::render_to_svg(&mut keyboard);
        assert!(
            focused.lines().any(|line| line.contains("stroke-width=\"2\"")),
            "keyboard focus must paint the ring around the current tab: {focused}"
        );

        let mut pointer = make();
        pointer.handle_event(&Event::FocusGained { reason: crate::event::FocusReason::Pointer });
        let unfocused = crate::widget::svg::render_to_svg(&mut pointer);
        assert!(
            !unfocused.lines().any(|line| line.contains("stroke-width=\"2\"")),
            "pointer focus must not paint a ring: {unfocused}"
        );
    }

    /// The close glyph is smaller than the platform hit floor, so the region that closes a
    /// tab reaches past the painted mark — but never into a neighbouring tab.
    #[test]
    fn a_press_beside_the_close_glyph_still_closes_through_the_hit_floor() {
        let mut bar = TabBar::new(Rect::new(0, 0, 200, 24));
        bar.add_tab("One".to_string());
        bar.add_tab("Two".to_string());
        bar.set_closable(true);

        let closed = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        bar.tab_close_requested.connect({
            let flag = std::sync::Arc::clone(&closed);
            move |_| flag.store(true, std::sync::atomic::Ordering::SeqCst)
        });

        let close = bar.close_rect(0).expect("a closable tab draws a close glyph");
        let tab = bar.tab_rect(0).expect("the tab has a rectangle");
        // Start on the second tab so a close that also selects would be observable.
        bar.set_current_index(1);
        // Five pixels left of the glyph: outside the painted 12 px mark, inside the expanded
        // hit region, and still inside the tab that owns it.
        let pos = Point::new(close.x - 5, close.y + close.height as i32 / 2);
        assert!(!close.contains(pos), "the fixture press must miss the painted glyph");
        assert!(tab.contains(pos), "the fixture press must stay on the owning tab");

        bar.handle_event(&Event::MousePress { pos, button: 1, modifiers: 0 });
        assert!(
            closed.load(std::sync::atomic::Ordering::SeqCst),
            "the hit floor must make the close button reachable beside its glyph"
        );
        assert_eq!(bar.current_index(), Some(1), "a close must not also select the tab");
    }

    /// Arrows move the selection, Delete requests a close, and clicks report their tab.
    #[test]
    fn the_keyboard_and_click_signal_are_wired() {
        use crate::event::{key_codes, Event, EventHandler};
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut bar = TabBar::new(Rect::new(0, 0, 300, 30));
        bar.add_tab("One".to_string());
        bar.add_tab("Two".to_string());
        bar.add_tab("Three".to_string());
        bar.set_current_index(0);
        bar.set_closable(true);

        let clicks = Arc::new(AtomicUsize::new(0));
        let closes = Arc::new(AtomicUsize::new(0));
        let c = Arc::clone(&clicks);
        let d = Arc::clone(&closes);
        bar.tab_bar_clicked.connect(move |_| {
            c.fetch_add(1, Ordering::SeqCst);
        });
        bar.tab_close_requested.connect(move |_| {
            d.fetch_add(1, Ordering::SeqCst);
        });

        bar.handle_event(&Event::key_press(key_codes::RIGHT, 0));
        assert_eq!(bar.current_index(), Some(1));
        bar.handle_event(&Event::key_press(key_codes::END, 0));
        assert_eq!(bar.current_index(), Some(2));
        bar.handle_event(&Event::key_press(key_codes::LEFT, 0));
        assert_eq!(bar.current_index(), Some(1));
        bar.handle_event(&Event::key_press(key_codes::HOME, 0));
        assert_eq!(bar.current_index(), Some(0));

        bar.handle_event(&Event::key_press(key_codes::DELETE, 0));
        assert_eq!(closes.load(Ordering::SeqCst), 1);

        let rect = bar.tab_rect(1).expect("the tab has a box");
        bar.handle_event(&Event::mouse_press(rect.x + 2, rect.y + 2, 1));
        assert_eq!(clicks.load(Ordering::SeqCst), 1);
    }

    /// A drag reports its live target before the release commits the move.
    #[test]
    fn a_drag_reports_its_target_before_release() {
        use crate::event::{Event, EventHandler};

        let mut bar = TabBar::new(Rect::new(0, 0, 400, 30));
        bar.add_tab("One".to_string());
        bar.add_tab("Two".to_string());
        bar.add_tab("Three".to_string());
        bar.set_movable(true);

        let first = bar.tab_rect(0).expect("tab 0 has a box");
        let third = bar.tab_rect(2).expect("tab 2 has a box");
        bar.handle_event(&Event::mouse_press(first.x + 2, first.y + 2, 1));
        bar.handle_event(&Event::mouse_move(third.x + 2, third.y + 2));
        assert_eq!(bar.drag_target(), Some(2), "the preview follows the pointer");
        assert!(bar.is_dragging());
        bar.handle_event(&Event::mouse_release(third.x + 2, third.y + 2, 1));
        assert!(!bar.is_dragging());
        assert_eq!(bar.drag_target(), None);
        assert_eq!(bar.tab_text(2), Some("One"), "the release committed");
    }

    /// Scroll buttons are opt-in, appear only on overflow, and shift the run.
    #[test]
    fn scroll_buttons_shift_an_overflowing_strip() {
        use crate::event::{Event, EventHandler};

        let mut bar = TabBar::new(Rect::new(0, 0, 80, 30));
        for index in 0..6 {
            bar.add_tab(format!("Tab {index}"));
        }
        bar.set_tab_min_width(60);
        assert!(bar.is_overflowing());
        assert!(bar.scroll_button_rects().is_none(), "off by default");

        bar.set_scroll_buttons(true);
        let (_left, right) = bar.scroll_button_rects().expect("the buttons appear");
        bar.handle_event(&Event::mouse_press(right.x + 2, right.y + 2, 1));
        assert_eq!(bar.scroll_offset(), 1);
        bar.handle_event(&Event::mouse_press(right.x + 2, right.y + 2, 1));
        assert_eq!(bar.scroll_offset(), 2);
        bar.set_scroll_buttons(false);
        assert_eq!(bar.scroll_offset(), 0, "turning them off resets the run");
    }

    /// Expanding stretches the tabs; auto-hide hides a one-tab strip; the other
    /// presentation options are opt-in.
    #[test]
    fn presentation_options_are_opt_in() {
        use crate::event::{Event, EventHandler};

        let mut bar = TabBar::new(Rect::new(0, 0, 300, 30));
        bar.add_tab("One".to_string());
        bar.add_tab("Two".to_string());
        let natural = bar.tab_rect(0).expect("tab 0").width;
        bar.set_expanding(true);
        assert!(bar.tab_rect(0).expect("tab 0").width > natural, "expanding widens");
        assert_eq!(bar.selection_behavior_on_remove(), SelectionBehaviorOnRemove::SelectPrevious);
        bar.set_selection_behavior_on_remove(SelectionBehaviorOnRemove::SelectNext);
        bar.set_current_index(0);
        bar.remove_tab(0);
        assert_eq!(bar.current_index(), Some(0), "SelectNext keeps the same slot");

        assert_eq!(bar.elide_mode(), TabElideMode::Right);
        bar.set_elide_mode(TabElideMode::None);
        assert_eq!(bar.elide_mode(), TabElideMode::None);
        assert_eq!(bar.style_variant(), TabBarStyle::Standard);
        bar.set_style_variant(TabBarStyle::Segmented);
        assert_eq!(bar.style_variant(), TabBarStyle::Segmented);

        assert!(!bar.auto_hide());
        bar.set_auto_hide(true);
        assert!(bar.auto_hide());
        // With one tab left, auto-hide ignores pointer input.
        bar.handle_event(&Event::mouse_press(5, 5, 1));
        assert_eq!(bar.current_index(), Some(0));
    }

    /// A wheel over an overflowing strip walks the visible run (ADV-13).
    #[test]
    fn the_wheel_scrolls_an_overflowing_strip() {
        let mut bar = TabBar::new(Rect::new(0, 0, 60, 24));
        for index in 0..6 {
            bar.add_tab(format!("Tab {index}"));
        }
        bar.set_scroll_buttons(true);
        assert!(bar.is_overflowing());
        assert_eq!(bar.scroll_offset(), 0);
        bar.handle_event(&Event::Wheel { delta: Point::new(0, 1), modifiers: 0 });
        assert_eq!(bar.scroll_offset(), 1, "a notch advances the run");
        bar.handle_event(&Event::Wheel { delta: Point::new(0, -1), modifiers: 0 });
        assert_eq!(bar.scroll_offset(), 0);
    }

    /// The middle button closes the tab under the pointer through the close request (ADV-13).
    #[test]
    fn the_middle_button_requests_a_close() {
        use std::sync::{Arc, Mutex};
        let mut bar = TabBar::new(Rect::new(0, 0, 400, 24));
        bar.add_tab("One".to_string());
        bar.add_tab("Two".to_string());
        bar.set_closable(true);
        let closed = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&closed);
        bar.tab_close_requested.connect(move |index| sink.lock().unwrap().push(*index));

        let rect = bar.tab_rect(1).expect("the second tab has a box");
        bar.handle_event(&Event::mouse_press(rect.x + 2, rect.y + 2, 2));
        assert_eq!(closed.lock().unwrap().as_slice(), &[1]);
    }

    /// The left and middle elide modes keep the identifying end of a title (ADV-12).
    #[test]
    fn left_and_middle_elision_keep_the_identifying_end() {
        let mut bar = TabBar::new(Rect::new(0, 0, 400, 24));
        bar.add_tab("report-2026-10.csv".to_string());
        let tab_rect = Rect::new(0, 0, 60, 24);
        let mut measure = |text: &str| text.chars().count() as u32 * 7;

        bar.set_elide_mode(TabElideMode::Left);
        let left = bar.fitted_title(0, tab_rect, &mut measure).into_owned();
        assert!(left.starts_with('\u{2026}'), "the leading end is the elided one: {left}");
        assert!(left.ends_with("csv"), "the suffix survives: {left}");

        bar.set_elide_mode(TabElideMode::Middle);
        let middle = bar.fitted_title(0, tab_rect, &mut measure).into_owned();
        assert!(middle.contains('\u{2026}'));
        assert!(middle.starts_with("rep"), "the head survives: {middle}");
        // The tail characters come from the end of the title (the split is half/half, so how many
        // survive depends on the budget, but the last one is the title's own last character).
        assert!(middle.ends_with('v'), "the tail survives: {middle}");

        // Right is still the default and still keeps the head.
        bar.set_elide_mode(TabElideMode::Right);
        let right = bar.fitted_title(0, tab_rect, &mut measure).into_owned();
        assert!(right.starts_with("rep"));
    }

    /// `tabs` round-trips through the record property, including the per-tab fields.
    ///
    /// This is the payload channel `add_tab`/`remove_tab` never had (B26.03).
    #[test]
    fn tabs_round_trip_through_the_property() {
        use crate::widget::capability::{RecordList, RecordValue, EMPTY_RECORD_SCHEMA};

        let mut bar = TabBar::new(Rect::new(0, 0, 320, 32));
        let list = RecordList::from_items(
            &EMPTY_RECORD_SCHEMA,
            vec![
                RecordValue::new()
                    .with("title", CapabilityValue::String(String::from("General")))
                    .with("tooltip", CapabilityValue::String(String::from("General settings"))),
                RecordValue::new()
                    .with("title", CapabilityValue::String(String::from("Advanced")))
                    .with("enabled", CapabilityValue::Bool(false)),
            ],
        );
        bar.set("tabs", CapabilityValue::Records(list)).expect("a well-formed strip is accepted");

        assert_eq!(bar.tab_count(), 2);
        assert_eq!(bar.tab_text(0), Some("General"));
        assert_eq!(bar.tab(0).expect("present").tooltip(), "General settings");
        assert_eq!(bar.tab_enabled(1), Some(false));

        let CapabilityValue::Records(read_back) = bar.get("tabs").expect("the strip reads back")
        else {
            panic!("`tabs` must read as a record list");
        };
        assert_eq!(read_back.len(), 2);
        assert!(matches!(
            read_back.items()[0].get("tooltip"),
            Some(CapabilityValue::String(tooltip)) if tooltip == "General settings"
        ));
        assert!(matches!(read_back.items()[1].get("enabled"), Some(CapabilityValue::Bool(false))));
    }

    /// A malformed strip is refused and changes nothing.
    #[test]
    fn a_malformed_tab_list_is_refused_and_changes_nothing() {
        use crate::widget::capability::{RecordList, RecordValue, EMPTY_RECORD_SCHEMA};

        let mut bar = TabBar::new(Rect::new(0, 0, 320, 32));
        bar.add_tab(String::from("Existing"));
        let before: Vec<String> =
            (0..bar.tab_count()).filter_map(|i| bar.tab_text(i).map(String::from)).collect();

        let broken = vec![
            (
                "missing title",
                vec![RecordValue::new().with("enabled", CapabilityValue::Bool(true))],
            ),
            (
                "unknown field",
                vec![RecordValue::new()
                    .with("title", CapabilityValue::String(String::from("A")))
                    .with("badge", CapabilityValue::UInt(1))],
            ),
        ];

        for (reason, records) in broken {
            let list = RecordList::from_items(&EMPTY_RECORD_SCHEMA, records);
            let result = bar.set("tabs", CapabilityValue::Records(list));
            assert!(
                matches!(result, Err(CapabilityAccessError::ConstraintViolation { .. })),
                "{reason} must be refused with a pointable constraint violation (got {result:?})"
            );
            let after: Vec<String> =
                (0..bar.tab_count()).filter_map(|i| bar.tab_text(i).map(String::from)).collect();
            assert_eq!(after, before, "{reason} must change nothing");
        }
    }

    /// `tabs` is published as a writable record property with its schema.
    #[test]
    fn the_tabs_property_is_published_with_its_record_schema() {
        let bar = TabBar::new(Rect::new(0, 0, 320, 32));
        assert!(WidgetProperties::property_names(&bar).contains(&"tabs"));
        let schema = crate::widget::capability::TAB_BAR_PROPERTIES
            .iter()
            .find(|schema| schema.name == "tabs")
            .expect("`tabs` is declared by the capability table");
        assert_eq!(schema.value_kind, crate::widget::capability::PropertyValueKind::Records);
        assert!(schema.readable && schema.writable);
        let record_schema = schema.record_schema.expect("a `Records` schema names its shape");
        assert_eq!(record_schema.id_field, "title");
        assert_eq!(record_schema.names(), vec!["title", "tooltip", "enabled", "icon_source"]);
        assert!(!record_schema.ids_unique && !record_schema.ids_required);
        assert_eq!(record_schema, &TAB_BAR_TAB_SCHEMA);
    }

    /// A double click on empty strip space requests a new tab; on a tab it does not (ADV-009).
    #[test]
    fn a_double_click_on_empty_space_requests_a_new_tab() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let mut bar = TabBar::new(Rect::new(0, 0, 400, 24));
        bar.add_tab("First".to_string());
        let requests = Arc::new(AtomicUsize::new(0));
        let sink = Arc::clone(&requests);
        bar.new_tab_requested.connect(move || {
            sink.fetch_add(1, Ordering::SeqCst);
        });

        // The single tab ends well before x = 350.
        bar.handle_event(&Event::mouse_double_click(350, 12, 1));
        assert_eq!(requests.load(Ordering::SeqCst), 1, "empty space requests a tab");

        let tab = bar.tab_rect(0).expect("the tab has a box");
        bar.handle_event(&Event::mouse_double_click(tab.x + 2, tab.y + 2, 1));
        assert_eq!(requests.load(Ordering::SeqCst), 1, "a tab is not empty space");
    }
}
