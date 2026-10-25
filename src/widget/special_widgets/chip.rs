// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Chip widget.

use crate::core::{Color, Font, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::style::{MotionSlot, PropertyDriver};
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconName, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};
use std::collections::HashMap;

// The text-entry partner of this control is declared here so the two share one item contract and
// one pill renderer (COMP-H02).
#[path = "chip_input.rs"]
pub mod chip_input;
pub use chip_input::ChipInput;

/// A chip's leading avatar content.
///
/// A chip label is often an identity (a person, a tag, a filter), and the baseline chip
/// carries a small visual token beside it. The three variants are the three things a chip
/// can show without a decoded image: initials, a colour swatch, or a built-in icon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChipAvatar {
    /// One or two letters drawn in the avatar slot.
    Initials(String),
    /// A filled colour swatch.
    Color(Color),
    /// A built-in vector icon.
    Icon(IconName),
}

impl ChipAvatar {
    /// The record-token spelling, so the avatar survives a property round trip.
    fn as_token(&self) -> String {
        match self {
            ChipAvatar::Initials(text) => format!("initials:{text}"),
            ChipAvatar::Color(color) => format!("color:{}", color.to_hex_rgba()),
            ChipAvatar::Icon(icon) => format!("icon:{}", icon.as_str()),
        }
    }

    fn from_token(token: &str) -> Option<Self> {
        if let Some(text) = token.strip_prefix("initials:") {
            return Some(ChipAvatar::Initials(text.to_string()));
        }
        if let Some(raw) = token.strip_prefix("color:") {
            return Color::parse_hex(raw).map(ChipAvatar::Color);
        }
        if let Some(name) = token.strip_prefix("icon:") {
            return IconName::from_name(name).map(ChipAvatar::Icon);
        }
        None
    }
}

/// One chip item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipItem {
    /// Stable item id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Whether the chip is selected.
    pub selected: bool,
    /// Optional leading avatar, shown in the same slot as the typed icon.
    pub avatar: Option<ChipAvatar>,
    /// Whether the item accepts toggling, closing and dragging.
    ///
    /// A mixed list is the normal case for a token input: an address that was already billed is
    /// read-only beside one the reader is still choosing.
    pub enabled: bool,
    /// Per-item close override. `None` follows the control's `show_close`.
    pub closable: Option<bool>,
}

impl ChipItem {
    /// Creates a new chip item.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            selected: false,
            avatar: None,
            enabled: true,
            closable: None,
        }
    }

    /// Sets the item's leading avatar.
    pub fn with_avatar(mut self, avatar: ChipAvatar) -> Self {
        self.avatar = Some(avatar);
        self
    }

    /// Marks the item read-only (`false`) or interactive (`true`).
    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Overrides whether the item paints and honours a close affordance.
    pub fn with_closable(mut self, closable: bool) -> Self {
        self.closable = Some(closable);
        self
    }

    /// Whether the item's close affordance is offered.
    fn close_allowed(&self, show_close: bool) -> bool {
        self.closable.unwrap_or(show_close) && self.enabled
    }
}

/// The declared shape of one chip item, as a record (BLUE26 B26.09).
///
/// # Why the control declares this itself
///
/// This is the row shape of the `items` property, and the control is what owns both ends of it: the
/// field names are `ChipItem`'s own field names, and `id` is the identity a selection is held by. A
/// table elsewhere would be a second declaration of the struct — the drift this crate's single-source
/// rule exists to prevent — so it lives beside the struct it describes.
///
/// `id` and `label` are required because [`ChipItem::new`] requires them; `selected` is optional
/// because a document that does not mention it means "not selected", which is the same state
/// `ChipItem::new` starts in.
pub const CHIP_ITEM_SCHEMA: crate::widget::capability::RecordSchema =
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
            crate::widget::capability::RecordField::optional(
                "selected",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            // A chip's leading icon, as the same structured descriptor a `Button` publishes. The
            // field is optional because a chip with no icon is the normal case, and the value is
            // `IconSpec` (not decomposed into `kind`/`name` fields) so the one-of-ness of
            // built-in-versus-resource is checked by the value's own parser rather than left to an
            // unenforceable "declare both, use one" convention.
            crate::widget::capability::RecordField::optional(
                "icon_source",
                crate::widget::capability::PropertyValueKind::IconSpec,
            ),
            // A chip's leading avatar (initials, colour swatch or icon), encoded as a string
            // token so every property value kind the record layer knows can carry it.
            crate::widget::capability::RecordField::optional(
                "avatar",
                crate::widget::capability::PropertyValueKind::String,
            ),
            // Whether the item is interactive. Optional, because a document that does not mention
            // it describes the ordinary case: an enabled chip.
            crate::widget::capability::RecordField::optional(
                "enabled",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
            // A per-item close override, for a list that mixes removable and read-only tokens.
            crate::widget::capability::RecordField::optional(
                "closable",
                crate::widget::capability::PropertyValueKind::Bool,
            ),
        ],
        id_field: "id",
        ids_unique: true,
        ids_required: true,
    };

/// The record form of one chip item: `id`, `label` and (only when it is set) `selected`.
///
/// `selected: false` is written out rather than omitted, so a read of `Chip::items` and a read of the
/// `items` property agree on the field set: a consumer that builds a table from the property does not
/// have to treat a differing arity as a different shape.
fn chip_item_record(
    item: &ChipItem,
    icon_source: Option<IconSource>,
) -> Result<crate::widget::capability::RecordValue, CapabilityAccessError> {
    let mut record = crate::widget::capability::RecordValue::new()
        .with("id", CapabilityValue::String(item.id.clone()))
        .with("label", CapabilityValue::String(item.label.clone()))
        .with("selected", CapabilityValue::Bool(item.selected))
        .with("enabled", CapabilityValue::Bool(item.enabled));
    if let Some(closable) = item.closable {
        record = record.with("closable", CapabilityValue::Bool(closable));
    }
    if let Some(avatar) = item.avatar.as_ref() {
        record = record.with("avatar", CapabilityValue::String(avatar.as_token()));
    }
    match icon_source {
        // A chip with no icon is the normal case, and "no `icon_source` field" is how a document
        // says so; the `set` half reads the missing field the same way.
        None => {}
        Some(IconSource::Builtin(name)) => {
            record = record.with(
                "icon_source",
                CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(name)),
            );
        }
        // A borrowed static geometry has no document representation, so this read **refuses** it
        // rather than leaving the field out. Leaving it out would be indistinguishable from "this
        // chip has no icon", so a designer round-trip would silently drop the item's icon and the
        // next `set` would erase it — the loss this crate's property layer refuses to make. The
        // control-level `icon_source` property returns the same error for the same reason.
        Some(IconSource::StaticGeometry(_)) => {
            return Err(CapabilityAccessError::IconSourceNotPersistable)
        }
    }
    Ok(record)
}

/// The item a record describes, or the reason it cannot be read as one.
///
/// Refuses a missing or wrongly-typed `id`/`label` rather than defaulting them: a chip with no id
/// cannot be addressed by a selection, and one with no label cannot be drawn — inventing either would
/// produce a control the document did not ask for.
fn chip_item_from_record(
    record: &crate::widget::capability::RecordValue,
) -> Result<(ChipItem, Option<IconSource>), CapabilityAccessError> {
    let text = |name: &str| match record.get(name) {
        Some(CapabilityValue::String(value)) => Ok(value.clone()),
        _ => Err(CapabilityAccessError::TypeMismatch),
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
    let selected = match record.get("selected") {
        None | Some(CapabilityValue::Null) => false,
        Some(CapabilityValue::Bool(flag)) => *flag,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let avatar = match record.get("avatar") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::String(token)) => {
            Some(ChipAvatar::from_token(token).ok_or(CapabilityAccessError::TypeMismatch)?)
        }
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let enabled = match record.get("enabled") {
        None | Some(CapabilityValue::Null) => true,
        Some(CapabilityValue::Bool(flag)) => *flag,
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    let closable = match record.get("closable") {
        None | Some(CapabilityValue::Null) => None,
        Some(CapabilityValue::Bool(flag)) => Some(*flag),
        Some(_) => return Err(CapabilityAccessError::TypeMismatch),
    };
    Ok((
        ChipItem { id: text("id")?, label: text("label")?, selected, avatar, enabled, closable },
        icon_source,
    ))
}

/// Square side of a chip item's leading icon slot, in logical pixels.
const CHIP_ICON_SIDE: u32 = 14;
/// Gap between that slot and the chip's label.
const CHIP_ICON_GAP: u32 = 4;
/// Width reserved for a chip's close affordance, including its gap.
const CHIP_CLOSE_SLOT: i32 = 14;
/// Width of the `+N` summary cell that stands in for chips that do not fit.
const CHIP_OVERFLOW_WIDTH: i32 = 34;

/// The laid-out chips and the inputs the layout was derived from.
///
/// `chip_layout` used to re-measure every label on each call, and a single `draw` alone calls it
/// three times (chips, overflow cell, drag marker) on top of every pointer hit test. A chips row
/// is a pure function of the items, the band it occupies and the resolved font, so it is computed
/// once and reused until one of those changes.
struct ChipLayout {
    band: Rect,
    font: Font,
    show_close: bool,
    padding: i32,
    spacing: i32,
    chips: Vec<(usize, Rect)>,
    overflow: Option<(Rect, usize)>,
    /// The `+N` caption, built with the layout because the hidden count is a layout product.
    overflow_label: String,
}

impl ChipLayout {
    /// Whether this layout still describes the inputs in hand.
    fn matches(
        &self,
        band: Rect,
        font: &Font,
        show_close: bool,
        padding: i32,
        spacing: i32,
    ) -> bool {
        self.band == band
            && &self.font == font
            && self.show_close == show_close
            && self.padding == padding
            && self.spacing == spacing
    }
}

/// Horizontal chip list with single or multi select mode.
pub struct Chip {
    base: BaseWidget,
    items: Vec<ChipItem>,
    multi_select: bool,
    focused_index: Option<usize>,
    chip_padding: i32,
    chip_spacing: i32,
    /// Per-item typed icon content, keyed by the item's **stable id**.
    ///
    /// `ChipItem`'s fields are public, so a caller may construct and mutate items without this
    /// control observing it; the override is therefore keyed by identity rather than by position,
    /// and [`Chip`] prunes it whenever the item list changes so an id can never name an item that
    /// no longer exists (blue27 §19.6). A hash map rather than a vector of pairs: the paint path
    /// looked each of N chips up by scanning M overrides, which made a long list with several
    /// overrides quadratic per frame.
    item_icon_sources: HashMap<String, IconSource>,
    /// Emitted when chip selection changes. Payload is chip id.
    pub chip_toggled: Signal1<String>,
    /// Emitted with the id of a chip removed through its close affordance.
    pub chip_removed: Signal1<String>,
    /// Emitted with the id of a chip whose position the user changed.
    pub chip_reordered: Signal1<String>,
    /// Emitted with the number of chips folded into the `+N` summary cell when it is pressed.
    pub overflow_activated: Signal1<usize>,
    /// Emitted when the chip under the pointer changes: its id on entry, `None` on exit.
    pub chip_hovered: Signal1<Option<String>>,
    /// The chip that started a shift-selection run.
    selection_anchor: Option<usize>,
    /// Whether the row flows onto further rows instead of folding the tail into `+N`.
    wrap: bool,
    /// The chip under the pointer.
    hovered_index: Option<usize>,
    /// Whether each chip paints a close affordance.
    show_close: bool,
    /// Whether a chip can be dragged to a new position.
    reorder_enabled: bool,
    /// The chip being dragged, and the slot it would land in.
    drag_source: Option<usize>,
    drag_target: Option<usize>,
    /// Whether a selection change slides the pill between chips.
    animate_selection: bool,
    /// The sliding pill's progress and the chip it started from.
    pill_slide: PropertyDriver,
    pill_from: usize,
    /// The cached row layout, rebuilt when the items, band, font or chrome inputs change.
    layout_cache: Option<ChipLayout>,
}

impl Chip {
    /// Creates empty chip widget.
    pub fn new(geometry: Rect) -> Self {
        Self {
            base: BaseWidget::new(WidgetKind::Chip, geometry, "Chip"),
            items: Vec::new(),
            multi_select: false,
            focused_index: None,
            chip_padding: dimensions::CHIP_PADDING_H as i32,
            chip_spacing: 6,
            item_icon_sources: HashMap::new(),
            chip_toggled: Signal1::new(),
            chip_removed: Signal1::new(),
            chip_reordered: Signal1::new(),
            overflow_activated: Signal1::new(),
            chip_hovered: Signal1::new(),
            selection_anchor: None,
            wrap: false,
            hovered_index: None,
            show_close: false,
            reorder_enabled: false,
            drag_source: None,
            drag_target: None,
            animate_selection: false,
            pill_slide: PropertyDriver::at(1.0, MotionSlot::Normal),
            pill_from: 0,
            layout_cache: None,
        }
    }

    /// Shows or hides the per-chip close affordance.
    pub fn set_show_close(&mut self, show: bool) {
        if self.show_close == show {
            return;
        }
        self.show_close = show;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns whether chips paint a close affordance.
    pub fn show_close(&self) -> bool {
        self.show_close
    }

    /// Enables pointer dragging of chips.
    pub fn set_reorder_enabled(&mut self, enabled: bool) {
        if self.reorder_enabled == enabled {
            return;
        }
        self.reorder_enabled = enabled;
        self.base.request_redraw();
    }

    /// Returns whether chips can be reordered by dragging.
    pub fn reorder_enabled(&self) -> bool {
        self.reorder_enabled
    }

    /// Enables the sliding selection pill.
    pub fn set_animate_selection(&mut self, animate: bool) {
        if self.animate_selection == animate {
            return;
        }
        self.animate_selection = animate;
        if !animate {
            self.pill_slide.jump_to(1.0);
        }
        self.base.request_redraw();
    }

    /// Returns whether the selection pill animates between chips.
    pub fn animate_selection(&self) -> bool {
        self.animate_selection
    }

    /// The chip under the pointer, if any.
    pub fn hovered_index(&self) -> Option<usize> {
        self.hovered_index
    }

    /// Removes the chip at `index`, publishing its id.
    ///
    /// Returns `false` when the index is out of bounds. The close affordance and a host calling
    /// this method share the same path, so keyboard and pointer removal cannot diverge.
    pub fn remove_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        let removed = self.items.remove(index);
        self.item_icon_sources.remove(&removed.id);
        self.focused_index =
            if self.items.is_empty() { None } else { Some(index.min(self.items.len() - 1)) };
        self.hovered_index = None;
        crate::style::reapply_active_theme_state(self);
        self.layout_cache = None;
        self.chip_removed.emit(removed.id);
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// Moves the chip at `from` into position `to`, publishing its id.
    ///
    /// Both indices are item positions; an out-of-range index is a no-op that returns `false`.
    pub fn move_item(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return false;
        }
        let item = self.items.remove(from);
        let id = item.id.clone();
        self.items.insert(to, item);
        self.focused_index = Some(to);
        self.layout_cache = None;
        self.chip_reordered.emit(id);
        self.base.request_redraw();
        true
    }

    /// Replaces all chip items.
    ///
    /// The single-select normalisation below can **clear** a selection, so this is a state change
    /// and not merely a content change — see [`Chip::toggle_index`] for why the theme has to be
    /// re-resolved on one.
    pub fn set_items(&mut self, items: Vec<ChipItem>) {
        self.items = items;
        // An override may only name an item that still exists. Dropping the ones whose id is gone
        // is what stops a later item that happens to reuse an id from inheriting the removed one's
        // icon (blue27 §19.6: an explicit removal must not resurrect old presentation).
        let live: Vec<String> = self.items.iter().map(|item| item.id.clone()).collect();
        self.item_icon_sources.retain(|id, _| live.contains(id));
        self.focused_index = if self.items.is_empty() { None } else { Some(0) };
        if !self.multi_select {
            // Keep only first selected chip in single-select mode.
            let mut selected_seen = false;
            for item in &mut self.items {
                if item.selected {
                    if selected_seen {
                        item.selected = false;
                    }
                    selected_seen = true;
                }
            }
        }
        crate::style::reapply_active_theme_state(self);
        self.layout_cache = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Returns all chip items.
    pub fn items(&self) -> &[ChipItem] {
        &self.items
    }

    /// Assigns typed icon content to the chip item owning `id`.
    ///
    /// The item's stable id is the identity this collection already has, so no second selection
    /// model is introduced. A missing or duplicated id is reported instead of silently targeting
    /// the wrong chip, and `None` clears the icon rather than revealing an older one.
    pub fn set_item_icon_source(
        &mut self,
        id: &str,
        source: Option<IconSource>,
    ) -> Result<(), crate::widget::IconItemError> {
        let mut matches = 0usize;
        for item in &self.items {
            if item.id == id {
                matches += 1;
            }
        }
        match matches {
            0 => return Err(crate::widget::IconItemError::UnknownItemId { id: String::from(id) }),
            1 => {}
            _ => {
                return Err(crate::widget::IconItemError::DuplicateItemId { id: String::from(id) })
            }
        }
        self.item_icon_sources.retain(|existing, _| existing != id);
        if let Some(source) = source {
            self.item_icon_sources.insert(String::from(id), source);
        }
        // An icon is part of the chip's box, so the row has to be measured again, not just redrawn.
        self.layout_cache = None;
        self.base.request_layout();
        self.base.request_redraw();
        Ok(())
    }

    /// Returns the typed icon source assigned to the item owning `id`.
    pub fn item_icon_source(&self, id: &str) -> Option<IconSource> {
        self.item_icon_sources.get(id).copied()
    }

    /// Returns the typed icon source for `item`, if one is assigned.
    fn icon_source_of(&self, item: &ChipItem) -> Option<IconSource> {
        self.item_icon_source(&item.id)
    }

    /// Replaces the whole id-keyed override map, dropping any entry the new list does not name.
    fn adopt_icon_sources(&mut self, sources: Vec<(String, IconSource)>) {
        self.item_icon_sources = sources.into_iter().collect();
        let live: Vec<String> = self.items.iter().map(|item| item.id.clone()).collect();
        self.item_icon_sources.retain(|id, _| live.contains(id));
        self.layout_cache = None;
    }

    /// Enables/disables multi-select mode.
    pub fn set_multi_select(&mut self, multi_select: bool) {
        if self.multi_select == multi_select {
            return;
        }
        self.multi_select = multi_select;
        if !self.multi_select {
            let mut selected_seen = false;
            for item in &mut self.items {
                if item.selected {
                    if selected_seen {
                        item.selected = false;
                    }
                    selected_seen = true;
                }
            }
        }
        // Dropping out of multi-select can clear a selection, which changes the state
        // `widget_state` reports — so the theme is re-resolved here too.
        crate::style::reapply_active_theme_state(self);
        self.base.request_redraw();
    }

    /// Returns whether multi-select is enabled.
    pub fn multi_select(&self) -> bool {
        self.multi_select
    }

    /// Returns focused chip index.
    pub fn focused_index(&self) -> Option<usize> {
        self.focused_index.filter(|index| *index < self.items.len())
    }

    /// Returns ids of selected chips.
    pub fn selected_ids(&self) -> Vec<&str> {
        self.items.iter().filter(|item| item.selected).map(|item| item.id.as_str()).collect()
    }

    /// Toggles chip selection.
    ///
    /// # Why this re-resolves the theme
    ///
    /// A selection change flips this control between [`WidgetState::Normal`] and
    /// [`WidgetState::Selected`], and those are two different theme keys (`chip` and
    /// `"chip:selected"`). `apply_active_theme` runs once, inside the creation funnels, and a
    /// redraw re-runs `draw` rather than the theme application — so without this call the
    /// control kept the style it was built in, `chip:selected` resolved to `rgb(33,150,243)` and
    /// reached nothing, and this method was a visible no-op. See
    /// [`crate::style::reapply_active_theme_state`] for the measurement.
    ///
    /// Before the signal, so a handler that reads the style back sees the state it is being told
    /// about rather than the previous one.
    pub fn toggle_index(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        // A read-only item reports the refusal rather than toggling silently.
        if !self.items[index].enabled {
            return false;
        }

        // Captured before the mutation: the pill slides *from* the chip that was selected,
        // and asking afterwards would always answer with the chip just chosen.
        let previous_selected = self.items.iter().position(|item| item.selected);
        if self.multi_select {
            self.items[index].selected = !self.items[index].selected;
        } else {
            let next = !self.items[index].selected;
            for item in &mut self.items {
                item.selected = false;
            }
            self.items[index].selected = next;
        }

        crate::style::reapply_active_theme_state(self);
        let id = self.items[index].id.clone();
        if self.animate_selection && self.items[index].selected {
            if let Some(previous) = previous_selected {
                if previous != index {
                    self.pill_from = previous;
                    self.pill_slide.jump_to(0.0);
                    self.pill_slide.set_target(1.0);
                }
            }
        }
        self.chip_toggled.emit(id);
        self.base.request_redraw();
        true
    }

    /// Moves focus by signed delta.
    ///
    /// Disabled items are skipped: a focus stop that cannot be activated is a dead end for a
    /// keyboard user.
    pub fn move_focus(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.focused_index = None;
            return;
        }
        let max = self.items.len().saturating_sub(1) as isize;
        let mut next = (self.focused_index.unwrap_or(0) as isize + delta).clamp(0, max);
        while self.items.get(next as usize).is_some_and(|item| !item.enabled) {
            let stepped = next + delta.signum();
            if stepped < 0 || stepped > max || stepped == next {
                break;
            }
            next = stepped;
        }
        self.focused_index = Some(next as usize);
        self.base.request_redraw();
    }

    /// Moves focus to the first or the last enabled item.
    fn move_focus_edge(&mut self, last: bool) {
        let target = if last {
            self.items.iter().rposition(|item| item.enabled)
        } else {
            self.items.iter().position(|item| item.enabled)
        };
        if let Some(index) = target {
            self.focused_index = Some(index);
            self.base.request_redraw();
        }
    }

    /// Extends the selection from the shift anchor to the focused item.
    fn extend_selection_to_focus(&mut self) {
        let Some(focus) = self.focused_index.filter(|index| *index < self.items.len()) else {
            return;
        };
        let anchor = *self.selection_anchor.get_or_insert(focus);
        let (first, last) = if anchor <= focus { (anchor, focus) } else { (focus, anchor) };
        let mut changed = false;
        for index in first..=last {
            if self.items.get(index).is_some_and(|item| item.enabled) && !self.items[index].selected
            {
                self.items[index].selected = true;
                changed = true;
            }
        }
        if changed {
            crate::style::reapply_active_theme_state(self);
            if let Some(id) = self.items.get(focus).map(|item| item.id.clone()) {
                self.chip_toggled.emit(id);
            }
            self.base.request_redraw();
        }
    }

    /// The width a chip needs for `item`, using the shared text measurement.
    ///
    /// # Why the shared ruler (D09-CTRL-02)
    ///
    /// The box used `chars().count() * 8`, a flat per-character estimate that under-counts wide
    /// glyphs (CJK, `W`) relative to the real advances the painter uses. A label wider than its
    /// estimated box was then clipped/fitted earlier than necessary. Measuring with
    /// [`estimate_text_width`] — the same ruler `implicit_size` and the painter share — makes the box
    /// fit the text it actually draws.
    fn chip_width(&self, item: &ChipItem, padding: i32, font: &Font) -> i32 {
        // The **effective font** — the resolved theme/caller font — so the chip box and the label are
        // measured with the same font the row paints with (D09-STYLE-01 / D09-STYLE-02). The caller
        // resolved it once for the whole row, so a layout pass no longer re-resolves it per item.
        let icon = if self.icon_source_of(item).is_some() || item.avatar.is_some() {
            CHIP_ICON_SIDE + CHIP_ICON_GAP
        } else {
            0
        };
        let close = if item.close_allowed(self.show_close) { CHIP_CLOSE_SLOT } else { 0 };
        estimate_text_width(&item.label, font, 1.0) as i32 + padding * 2 + icon as i32 + close
    }

    /// Whether the row flows onto further rows instead of folding its tail into `+N`.
    pub fn wrap(&self) -> bool {
        self.wrap
    }

    /// Enables or disables multi-row flow.
    ///
    /// Off by default, so a caller that never asks keeps the single-row pixels it had. With it on, a
    /// row that runs out of width starts a new one and the `+N` summary disappears: the summary is
    /// what a single-row layout does instead of wrapping, and doing both would lose chips twice.
    pub fn set_wrap(&mut self, wrap: bool) {
        if self.wrap == wrap {
            return;
        }
        self.wrap = wrap;
        self.layout_cache = None;
        self.base.request_layout();
        self.base.request_redraw();
    }

    /// Sets a single item's close affordance independently of the control's switch.
    pub fn set_item_closable(&mut self, id: &str, closable: bool) -> bool {
        let Some(item) = self.items.iter_mut().find(|item| item.id == id) else {
            return false;
        };
        item.closable = Some(closable);
        self.layout_cache = None;
        self.base.request_layout();
        self.base.request_redraw();
        true
    }

    /// The band the chip row occupies, or the full rectangle when wrapping.
    fn row_band(&self) -> Rect {
        if self.wrap {
            self.geometry()
        } else {
            ControlMetrics::full_width_band(self.geometry(), dimensions::CHIP_HEIGHT)
        }
    }

    /// Rebuilds the cached layout when its inputs have changed.
    ///
    /// Called before any read. Keeping "refresh" and "read" separate is what lets the read take
    /// `&self`: the row is then evaluated once per frame instead of once per chip, per overflow
    /// cell and per hit test.
    fn ensure_layout(&mut self) {
        let band = self.row_band();
        let font = effective_font(self.style()).clone();
        let fresh = self.layout_cache.as_ref().is_some_and(|cache| {
            cache.matches(band, &font, self.show_close, self.chip_padding, self.chip_spacing)
        });
        if !fresh {
            self.layout_cache = Some(self.build_layout(band, font));
        }
    }

    /// The cached layout. [`Self::ensure_layout`] must have run first.
    fn cached_layout(&self) -> &ChipLayout {
        self.layout_cache.as_ref().expect("ensure_layout() populates the layout cache")
    }

    /// Lays the row out once.
    ///
    /// A chips row is a linear strip, so a single pass is enough. Chips that would start past the
    /// band's right edge are folded into the summary cell rather than painted outside the control.
    fn build_layout(&self, band: Rect, font: Font) -> ChipLayout {
        let mut layout = Vec::with_capacity(self.items.len());
        let mut x = band.x + dimensions::CHIP_PADDING_H as i32;
        let mut y = band.y;
        let right = band.x + band.width as i32;
        // A chip is its own height, clamped to the band only when the band is shorter than one
        // chip — the rule the single-row layout always followed.
        let chip_height =
            if self.wrap { dimensions::CHIP_HEIGHT.min(band.height.max(1)) } else { band.height };
        let mut overflow = None;
        for (index, item) in self.items.iter().enumerate() {
            let width =
                (self.chip_width(item, self.chip_padding, &font).max(10) as u32).min(band.width);
            if x + width as i32 > right {
                if self.wrap {
                    // Multi-row flow: start the next row at the same left inset. A chip wider than
                    // the whole band still gets one row of its own rather than looping forever.
                    x = band.x + dimensions::CHIP_PADDING_H as i32;
                    y += chip_height as i32 + self.chip_spacing / 2;
                } else {
                    overflow = Some((index, (right - x).max(0) as u32));
                    break;
                }
            }
            let available = (right - x).max(1) as u32;
            layout.push((index, Rect::new(x, y, width.min(available), chip_height)));
            x += width as i32 + self.chip_spacing;
        }
        let mut overflow_label = String::new();
        let overflow_cell = overflow.and_then(|(first_hidden, room)| {
            let hidden = self.items.len() - first_hidden;
            if hidden == 0 || room < 18 {
                return None;
            }
            // Fold the last visible chip back into the summary when there is no room for it.
            if let Some((_, last)) = layout.last().copied() {
                if last.x + last.width as i32 + 6 > right {
                    layout.pop();
                }
            }
            let start = layout.last().map_or(band.x, |(_, rect)| rect.x + rect.width as i32);
            let start = start + self.chip_spacing;
            let available = (right - start).max(1) as u32;
            let width = (CHIP_OVERFLOW_WIDTH as u32).min(available);
            let hidden = self.items.len() - layout.len();
            overflow_label = format!("+{hidden}");
            Some((Rect::new(start, band.y, width, chip_height), hidden))
        });
        ChipLayout {
            band,
            font,
            show_close: self.show_close,
            padding: self.chip_padding,
            spacing: self.chip_spacing,
            chips: layout,
            overflow: overflow_cell,
            overflow_label,
        }
    }

    /// The number of rows the items occupy when wrapping.
    ///
    /// Measured without touching the layout cache: `size_hint` takes `&self`, and the count is a
    /// pure function of the items and the band.
    fn wrapped_row_count(&self) -> usize {
        let rect = self.geometry();
        if rect.width == 0 || self.items.is_empty() {
            return 1;
        }
        let font = effective_font(self.style());
        let inset = dimensions::CHIP_PADDING_H as i32;
        let right = rect.x + rect.width as i32;
        let mut x = rect.x + inset;
        let mut rows = 1usize;
        for item in &self.items {
            let width =
                (self.chip_width(item, self.chip_padding, font).max(10) as u32).min(rect.width);
            if x + width as i32 > right && x > rect.x + inset {
                rows += 1;
                x = rect.x + inset;
            }
            x += width as i32 + self.chip_spacing;
        }
        rows
    }

    #[cfg(test)]
    fn chip_rect(&mut self, index: usize) -> Option<Rect> {
        self.ensure_layout();
        self.cached_layout().chips.iter().find(|(item, _)| *item == index).map(|(_, rect)| *rect)
    }

    /// The cached row in the shape the layout tests read it.
    #[cfg(test)]
    fn chip_layout_with_overflow(&mut self) -> (Vec<(usize, Rect)>, Option<(Rect, usize)>) {
        self.ensure_layout();
        let layout = self.cached_layout();
        (layout.chips.clone(), layout.overflow)
    }

    fn hit_index(&mut self, pos: Point) -> Option<usize> {
        self.ensure_layout();
        self.cached_layout()
            .chips
            .iter()
            .find(|(index, chip)| {
                // A disabled chip is drawn but not a target: clicking one must fall through to
                // whatever is behind rather than toggling a read-only item.
                self.items.get(*index).is_some_and(|item| item.enabled)
                    && pos.x >= chip.x
                    && pos.x < chip.x + chip.width as i32
                    && pos.y >= chip.y
                    && pos.y < chip.y + chip.height as i32
            })
            .map(|(index, _)| *index)
    }

    /// Returns the hidden count when the pointer is on the `+N` summary cell.
    fn hit_overflow(&mut self, pos: Point) -> Option<usize> {
        self.ensure_layout();
        self.cached_layout()
            .overflow
            .and_then(|(cell, hidden)| cell.contains_point(pos).then_some(hidden))
    }

    /// Returns the chip whose close affordance is under the pointer.
    fn hit_close(&mut self, pos: Point) -> Option<usize> {
        self.ensure_layout();
        let show_close = self.show_close;
        self.cached_layout().chips.iter().find_map(|(index, chip)| {
            let allowed = self.items.get(*index).is_some_and(|item| item.close_allowed(show_close));
            if !allowed {
                return None;
            }
            let close = Rect::new(
                chip.x + chip.width as i32 - CHIP_CLOSE_SLOT,
                chip.y,
                CHIP_CLOSE_SLOT as u32,
                chip.height,
            );
            (close.contains_point(pos) && chip.contains_point(pos)).then_some(*index)
        })
    }

    /// Returns the item position the pointer would drop a dragged chip into.
    fn drop_target(&mut self, pos: Point) -> Option<usize> {
        self.ensure_layout();
        let layout = self.cached_layout();
        if layout.chips.is_empty() {
            return None;
        }
        for (index, chip) in &layout.chips {
            if pos.x < chip.x + chip.width as i32 / 2 {
                return Some(*index);
            }
        }
        Some(layout.chips.len().saturating_sub(1))
    }
}

impl Widget for Chip {
    fn base(&self) -> &BaseWidget {
        &self.base
    }
    fn set_state_theme_hook(&mut self) {
        crate::style::reapply_active_theme_state(self);
    }

    fn base_mut(&mut self) -> &mut BaseWidget {
        &mut self.base
    }

    /// `Chip` paints itself, so it can be mounted into a native window.
    fn as_draw_mut(&mut self) -> Option<&mut dyn crate::widget::Draw> {
        Some(self)
    }

    fn size_hint(&self) -> crate::core::Size {
        if self.wrap {
            // A wrapped row's height grows with the number of rows it flows onto, so the hint has
            // to grow with it rather than staying one chip tall.
            let rows = self.wrapped_row_count().max(1) as u32;
            return crate::core::Size::new(80, dimensions::CHIP_HEIGHT * rows);
        }
        crate::core::Size::new(80, 24)
    }

    /// Advances the sliding selection pill.
    fn tick(&mut self, delta_ms: u32) -> bool {
        if !self.animate_selection {
            return false;
        }
        let moving = self.pill_slide.tick(delta_ms);
        if moving {
            self.base.request_redraw();
        }
        moving
    }

    /// Whether the selection pill still owes frames.
    fn is_animating(&self) -> bool {
        self.animate_selection && self.pill_slide.is_moving()
    }

    /// Reports `Selected` when at least one chip is chosen.
    ///
    /// # Why `Selected` and not `Checked`
    ///
    /// This control is a **list**, not a latch: several of its members can be chosen at once, and
    /// there is no single "on" for the control as a whole. `WidgetState::Selected` is precisely
    /// "one of the currently selected items in a collection", which is what a chip list is, and
    /// `Checked` would claim the control has one binary state it does not have.
    ///
    /// The preset keys follow the report: `chip:checked` was declared for a state this control
    /// never reaches (see `CheckBox::widget_state` for the same defect), so the keys are now
    /// `chip:selected`.
    ///
    /// # Where the key is consumed
    ///
    /// `draw` reads `chip:selected` for the **selected chip's own box and label** — that one chip's
    /// fill and ink, not the row. The row band is the base `chip` appearance, so selecting a chip
    /// changes one chip and leaves the rest of the row as it was. (The key was previously consumed
    /// at the wrong surface: the whole band took the `primary` fill and every other chip derived
    /// from it, which is why the documentation here had to be corrected rather than the code.)
    fn widget_state(&self) -> crate::style::WidgetState {
        use crate::style::WidgetState;
        if !self.base.is_enabled() {
            return WidgetState::Disabled;
        }
        if self.items.iter().any(|item| item.selected) {
            return WidgetState::Selected;
        }
        if self.base.is_pressed() {
            WidgetState::Pressed
        } else if self.base.is_hovered() {
            WidgetState::Hover
        } else if self.base.draws_focus_ring() {
            WidgetState::Focused
        } else {
            WidgetState::Normal
        }
    }

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
            "chip_toggled" => {
                Some(EventSignalRef::mapped("chip_toggled", &self.chip_toggled, |v| {
                    CapabilityValue::String(v.clone())
                }))
            }
            "chip_removed" => {
                Some(EventSignalRef::mapped("chip_removed", &self.chip_removed, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "chip_reordered" => {
                Some(EventSignalRef::mapped("chip_reordered", &self.chip_reordered, |v| {
                    crate::widget::capability::CapabilityValue::String(v.clone())
                }))
            }
            "overflow_activated" => {
                Some(EventSignalRef::mapped("overflow_activated", &self.overflow_activated, |v| {
                    crate::widget::capability::CapabilityValue::UInt(*v as u64)
                }))
            }
            _ => None,
        }
    }
}

/// `Chip`'s property contract, published under the `CheckListBox` kind.
///
/// `WidgetKind::CheckListBox` is the kind the capability layer pairs with this
/// control (`chip_capability`), which is why the old `CheckListBox` arms
/// downcast to `Chip`. `focused_index` is declared by `CHIP_PROPERTIES` but the
/// centralised reader never served it, so it is not published here either —
/// adding it would invent behaviour rather than preserve it.
impl WidgetProperties for Chip {
    fn get(&self, name: &str) -> Result<CapabilityValue, CapabilityAccessError> {
        match name {
            "items" => {
                Ok(CapabilityValue::Records(crate::widget::capability::RecordList::from_items(
                    &CHIP_ITEM_SCHEMA,
                    self.items()
                        .iter()
                        .map(|item| chip_item_record(item, self.icon_source_of(item)))
                        .collect::<Result<Vec<_>, CapabilityAccessError>>()?,
                )))
            }
            "item_count" => Ok(CapabilityValue::UInt(self.items().len() as u64)),
            "multi_select" => Ok(CapabilityValue::Bool(self.multi_select())),
            "focused_index" => match self.focused_index() {
                Some(index) => Ok(CapabilityValue::UInt(index as u64)),
                None => Ok(CapabilityValue::Null),
            },
            "selected_count" => Ok(CapabilityValue::UInt(self.selected_ids().len() as u64)),
            _ => base_property_get(self, name),
        }
    }

    fn set(&mut self, name: &str, value: CapabilityValue) -> Result<(), CapabilityAccessError> {
        match name {
            "items" => {
                let CapabilityValue::Records(list) = value else {
                    return Err(CapabilityAccessError::TypeMismatch);
                };
                // Checked in full **before** anything is replaced, so a refused update leaves the
                // live items untouched rather than half-swapped. A withdrawn or superseded pending
                // edit is therefore safe to abandon: nothing has to be rolled back, because nothing
                // was applied.
                list.validate_against(&CHIP_ITEM_SCHEMA).map_err(|violation| {
                    log::warn!("chip.items refused: {violation}");
                    // The property name and the reason travel back, because B26.02 requires a
                    // refusal to be pointable: a bare `TypeMismatch` cannot say *which record* and
                    // *which field* was wrong, and this is the layer that knows.
                    CapabilityAccessError::ConstraintViolation {
                        property: crate::compat::String::from("items"),
                        reason: crate::compat::format!("{violation}"),
                    }
                })?;
                let mut items = Vec::with_capacity(list.len());
                let mut icon_sources = Vec::new();
                for record in list.items() {
                    let (item, source) = chip_item_from_record(record)?;
                    if let Some(source) = source {
                        icon_sources.push((item.id.clone(), source));
                    }
                    items.push(item);
                }
                // `set_items` is the control's own normalisation (single-select collapses to one
                // selection), so the property write goes through it rather than restating the rule.
                self.set_items(items);
                // The document is the authority on what each row carries: a record that omits
                // `icon_source` means "this chip has no icon", not "keep whatever was there".
                self.adopt_icon_sources(icon_sources);
                Ok(())
            }
            "multi_select" => {
                self.set_multi_select(expect_bool(value)?);
                Ok(())
            }
            // Derived from the item list and the live selection.
            "item_count" | "focused_index" | "selected_count" => {
                Err(CapabilityAccessError::ReadOnlyProperty)
            }
            _ => base_property_set(self, name, value),
        }
    }

    fn property_names(&self) -> &'static [&'static str] {
        property_names_of![
            "items",
            "item_count",
            "multi_select",
            "focused_index",
            "selected_count",
            BASE_PROPERTY_NAMES
        ]
    }

    /// Runs one of the commands `chip` publishes.
    ///
    /// None of the published commands takes no argument: `toggle_index` names *which* chip to toggle,
    /// `move_focus` takes a signed delta whose direction cannot be inferred, and `set_items` takes the
    /// item list. All three are therefore refused as [`CapabilityAccessError::OutOfRange`] — the names
    /// are valid and the arguments are what is missing — rather than `UnknownCommand`, which would
    /// deny that the control has them.
    ///
    /// # Where `set_items`'s payload actually goes
    ///
    /// A command's signature carries no argument, so the item list travels through the **`items`
    /// property** instead ([`Self::set`]) — the same division B26.03 settled for `series` on the chart
    /// widgets, where a value-carrying operation is a property write and the command form is the
    /// no-argument half. A caller that reads this name and calls it gets `OutOfRange`, and the
    /// capability's `items` property is what it should read instead.
    fn command(&mut self, name: &str) -> Result<(), CapabilityAccessError> {
        match name {
            "toggle_index" | "move_focus" | "set_items" => Err(CapabilityAccessError::OutOfRange),
            _ => Err(CapabilityAccessError::UnknownCommand),
        }
    }
}

impl EventHandler for Chip {
    fn handle_event(&mut self, event: &Event) {
        self.base.handle_event(event);
        if !self.base.is_enabled() {
            return;
        }

        match event {
            Event::MouseMove { pos } => {
                let next = self.hit_index(*pos);
                if next != self.hovered_index {
                    self.hovered_index = next;
                    // The id travels with the transition, so a host can show a tooltip for the chip
                    // it is actually over without mapping a private index itself.
                    let id =
                        next.and_then(|index| self.items.get(index).map(|item| item.id.clone()));
                    self.chip_hovered.emit(id);
                    self.base.request_redraw();
                }
                if let Some(source) = self.drag_source {
                    if let Some(target) = self.drop_target(*pos) {
                        if Some(target) != self.drag_target {
                            self.drag_target = Some(target);
                            self.base.request_redraw();
                        }
                    }
                    let _ = source;
                }
            }
            Event::MouseLeave { .. } => {
                let hovered = self.hovered_index.take().is_some();
                let changed = hovered || self.drag_target.take().is_some();
                if hovered {
                    self.chip_hovered.emit(None);
                }
                if changed {
                    self.base.request_redraw();
                }
            }
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(hidden) = self.hit_overflow(*pos) {
                    self.overflow_activated.emit(hidden);
                    return;
                }
                if let Some(index) = self.hit_close(*pos) {
                    let _ = self.remove_index(index);
                    return;
                }
                if let Some(index) = self.hit_index(*pos) {
                    self.focused_index = Some(index);
                    if self.reorder_enabled {
                        self.drag_source = Some(index);
                        self.drag_target = Some(index);
                    } else {
                        let _ = self.toggle_index(index);
                    }
                }
            }
            Event::MouseRelease { pos, button: 1 } => {
                if let Some(source) = self.drag_source.take() {
                    if let Some(target) = self.drop_target(*pos) {
                        if target != source {
                            let _ = self.move_item(source, target);
                        } else {
                            self.focused_index = Some(source);
                            let _ = self.toggle_index(source);
                        }
                    }
                    self.drag_target = None;
                    self.base.request_redraw();
                }
            }
            Event::KeyPress { key, modifiers } => {
                let pressed = crate::shortcut::Modifiers::from_event_bits(*modifiers);
                let shift = pressed.contains(crate::shortcut::Modifiers::SHIFT);
                match *key {
                    37 => {
                        if shift && self.multi_select {
                            self.selection_anchor.get_or_insert(self.focused_index.unwrap_or(0));
                        }
                        self.move_focus(-1);
                        if shift && self.multi_select {
                            self.extend_selection_to_focus();
                        } else {
                            self.selection_anchor = None;
                        }
                    }
                    39 => {
                        if shift && self.multi_select {
                            self.selection_anchor.get_or_insert(self.focused_index.unwrap_or(0));
                        }
                        self.move_focus(1);
                        if shift && self.multi_select {
                            self.extend_selection_to_focus();
                        } else {
                            self.selection_anchor = None;
                        }
                    }
                    // Home/End reach the ends of the row, which a long list otherwise makes a long
                    // arrow-key walk.
                    36 => {
                        self.selection_anchor = None;
                        self.move_focus_edge(false);
                    }
                    35 => {
                        self.selection_anchor = None;
                        self.move_focus_edge(true);
                    }
                    13 | 32 => {
                        if let Some(index) = self.focused_index() {
                            let _ = self.toggle_index(index);
                        }
                    }
                    46 | 8 => {
                        // Delete/Backspace removes the focused chip through the same path as
                        // its close affordance; a read-only chip refuses.
                        if let Some(index) = self.focused_index() {
                            if self.items.get(index).is_some_and(|item| item.enabled) {
                                let _ = self.remove_index(index);
                            }
                        }
                    }
                    // Unknown key; ignore
                    _ => {}
                }
            }
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for Chip {
    fn draw(&mut self, context: &mut RenderContext) {
        // Measure the row once, before any style borrow is taken: the same layout serves the chip
        // loop, the overflow summary and the drag marker, instead of three full passes per frame.
        self.ensure_layout();
        // Chrome colours resolve explicit style first, then the theme's resolved
        // style for this control, and only then fall back to a literal. The theme
        // step is what makes a light/dark switch visible here; without it every
        // colour below was hardcoded and the switch changed nothing.
        //
        // Each `resolved_theme_style` call takes and releases the global manager's
        // lock internally, so no guard is held across the draw or across another
        // accessor (the mutex is not re-entrant).
        // `theme` is the theme's **base** `chip` appearance — not the control's own style, which
        // `reapply_active_theme_state` re-resolves to `chip:selected` as soon as *any* chip is
        // chosen (the whole control reports `Selected`; see `widget_state`). Reading the row's
        // base from `style` was the defect: one selected chip turned the entire band into the
        // declared selected fill and every other chip derived from it. The declared base of the
        // row is a fact about the theme and the row, so it comes from the theme.
        let style = self.base.style();
        let theme = crate::style::resolved_theme_style_shared("chip");
        let background = theme
            .as_ref()
            .and_then(|t| t.background_color)
            .or(style.background_color)
            .unwrap_or(Color::WHITE);
        let border = style
            .border_color
            .or_else(|| theme.as_ref().and_then(|t| t.border_color))
            .unwrap_or_else(|| crate::core::Color::border_of(background));
        let text_color =
            theme.as_ref().and_then(|t| t.text_color).or(style.text_color).unwrap_or(Color::BLACK);
        // One font for the whole row: resolving it inside the loop re-created the family string per
        // chip, and the avatar path resolved it twice per avatar chip.
        let font = effective_font(style);

        // `chip:selected` is the theme's **declared** fill for the selected chip, and it applies
        // to that one chip's box and ink — not to the row. It used to be consumed into the row
        // band, so selecting a single chip repainted the whole row in `primary` and derived every
        // other chip's fill from it: the wrong surface, and the module documentation claimed a
        // consumer "in both directions" that did not exist. The declared layer is read here and
        // used only inside the `item.selected` arm below.
        #[cfg(device_profile)]
        let selected_style = crate::style::resolved_theme_style_for_state(
            "chip",
            crate::style::WidgetState::Selected,
        );
        #[cfg(not(device_profile))]
        let selected_style: Option<crate::style::WidgetStyle> = None;

        // ── The row band actually painted ──
        //
        // `rect` is the area the control was *given*; a chip row's own chrome is one row of
        // chips, [`dimensions::CHIP_HEIGHT`] tall. Painting the control's surface across the
        // whole rectangle made a 240x120 census cell a full-bleed panel with no chip shape in
        // it at all — the defect this replaces — and put the row's background behind empty
        // space no chip could occupy. The band is centred, so it sits on the control's middle
        // line whatever height the caller supplies, which is the same derivation each chip
        // uses (`row_band`).
        // The row band is the *measure* of the chips, not a frame around them: the band used to
        // be filled and stroked, so a single chip in a 240 px control was drawn inside a
        // full-width rectangle with a vertical rule where the chip ended — a container with one
        // child, which is not what a chip row looks like. Each chip paints its own pill instead;
        // a chip is a rounded pill by definition, and a square box read as a table cell.

        for &(index, chip_rect) in &self.cached_layout().chips {
            let Some(item) = self.items.get(index) else {
                continue;
            };

            // Selection and focus are chrome states, so they are derived from the
            // resolved colours rather than from literals: a selected chip reads as
            // tinted toward the foreground on whatever background the theme picked.
            //
            // A selected chip prefers the theme's **declared** `chip:selected` fill and ink, and
            // falls back to the same tint derivation the other states use. The fallback keeps the
            // chip readable on a theme that declares no such key, which is why the derivation is
            // not replaced outright.
            let mut bg = if !item.enabled {
                // A read-only chip reads as present but inert: the weakest fill the other states
                // use, with a dimmed label below.
                background.blend(&text_color, 0.03)
            } else if item.selected {
                background.blend(&text_color, 0.22)
            } else if self.focused_index == Some(index) {
                background.blend(&text_color, 0.12)
            } else if self.hovered_index == Some(index) {
                background.blend(&text_color, 0.10)
            } else {
                background.blend(&text_color, 0.06)
            };
            let mut label_ink = if item.enabled { text_color } else { text_color.with_alpha(120) };
            // The pill's ink must contrast with the fill it is actually painted on. A selected
            // chip whose theme declares a fill but no ink kept the body text colour, so the
            // accent pill carried dark text on a dark blue fill (the census sample showed exactly
            // that). A declared ink still wins; otherwise the fill's own contrast colour does.
            if item.selected {
                if let Some(declared) = selected_style.as_ref() {
                    if let Some(declared_background) = declared.background_color {
                        bg = declared_background;
                    }
                    if let Some(declared_ink) = declared.text_color {
                        label_ink = declared_ink;
                    } else {
                        label_ink = bg.contrast_color();
                    }
                } else {
                    label_ink = bg.contrast_color();
                }
            }
            // A chip is a pill: the radius is half its height, clamped so a caller-supplied
            // short height cannot invert the corner.
            let pill_radius = (chip_rect.height / 2).max(1);
            context.fill_rounded_rect(chip_rect, pill_radius, bg);
            context.draw_rounded_rect_stroke(chip_rect, pill_radius, border, 1);
            // ── Avatar ──
            //
            // A typed icon wins; otherwise an avatar fills the same leading slot. Both are
            // charged in `chip_width`, so the label box and the slot cannot disagree.
            let mut label_box = chip_rect;
            if let Some(source) = self.icon_source_of(item) {
                let side = CHIP_ICON_SIDE.min(chip_rect.height.saturating_sub(4)).max(1);
                let icon_rect = Rect::new(
                    chip_rect.x + CHIP_ICON_GAP as i32,
                    chip_rect.y + (chip_rect.height.saturating_sub(side) / 2) as i32,
                    side,
                    side,
                );
                if let Err(error) =
                    crate::widget::paint_icon(context, source.into(), icon_rect, label_ink)
                {
                    log::warn!("chip item '{}' icon failed: {error:?}", item.id);
                }
                label_box.x += (CHIP_ICON_SIDE + CHIP_ICON_GAP) as i32;
                label_box.width = label_box.width.saturating_sub(CHIP_ICON_SIDE + CHIP_ICON_GAP);
            } else if let Some(avatar) = item.avatar.as_ref() {
                let side = CHIP_ICON_SIDE.min(chip_rect.height.saturating_sub(4)).max(1);
                let avatar_rect = Rect::new(
                    chip_rect.x + CHIP_ICON_GAP as i32,
                    chip_rect.y + (chip_rect.height.saturating_sub(side) / 2) as i32,
                    side,
                    side,
                );
                match avatar {
                    ChipAvatar::Initials(text) => {
                        context.fill_circle(
                            crate::core::Point::new(
                                avatar_rect.x + side as i32 / 2,
                                avatar_rect.y + side as i32 / 2,
                            ),
                            side / 2,
                            label_ink.with_alpha(60),
                        );
                        context.draw_text_fitted_cow(
                            avatar_rect,
                            text,
                            font,
                            label_ink,
                            HorizontalAlignment::Center,
                        );
                    }
                    ChipAvatar::Color(color) => {
                        context.fill_circle(
                            crate::core::Point::new(
                                avatar_rect.x + side as i32 / 2,
                                avatar_rect.y + side as i32 / 2,
                            ),
                            side / 2,
                            *color,
                        );
                    }
                    ChipAvatar::Icon(icon) => {
                        crate::widget::draw_icon_at(context, avatar_rect, label_ink, *icon);
                    }
                }
                label_box.x += (CHIP_ICON_SIDE + CHIP_ICON_GAP) as i32;
                label_box.width = label_box.width.saturating_sub(CHIP_ICON_SIDE + CHIP_ICON_GAP);
            }
            // The chip's label is centred through the shared primitive: `chip_rect.y +
            // chip_rect.height / 2` is the glyph box's *top* edge on the chip's middle line,
            // which drew the label half a line low. The label itself is host-supplied, and the
            // chip's *box* is already clamped to the band (`(width as u32).min(band.width)`), so
            // the ink is fitted into that same clamped box: a long label now elides inside its
            // chip instead of running past the box the chip was drawn as.
            context.draw_text_line_cow(
                label_box,
                &item.label,
                font,
                label_ink,
                HorizontalAlignment::Left,
            );

            // ── Close affordance ──
            //
            // Two strokes forming an ×, in the ink of the pill it sits on. The slot is charged
            // in `chip_width`, so the label was fitted into the room that is actually left.
            if item.close_allowed(self.show_close) {
                let cx = chip_rect.x + chip_rect.width as i32 - CHIP_CLOSE_SLOT / 2;
                let cy = chip_rect.y + chip_rect.height as i32 / 2;
                let arm = 3;
                let close_ink = label_ink.with_alpha(190);
                context.draw_line_stroke(
                    Point::new(cx - arm, cy - arm),
                    Point::new(cx + arm, cy + arm),
                    close_ink,
                    1,
                );
                context.draw_line_stroke(
                    Point::new(cx + arm, cy - arm),
                    Point::new(cx - arm, cy + arm),
                    close_ink,
                    1,
                );
            }
        }

        // ── Overflow summary ──
        if let Some((cell, _)) = self.cached_layout().overflow {
            let radius = (cell.height / 2).max(1);
            let fill = background.blend(&text_color, 0.06);
            context.fill_rounded_rect(cell, radius, fill);
            context.draw_rounded_rect_stroke(cell, radius, border, 1);
            // The caption was built with the layout: the hidden count is a layout product, so
            // formatting it here made the paint path recompute a layout answer.
            let text = self.cached_layout().overflow_label.as_str();
            context.draw_text_line_cow(cell, text, font, text_color, HorizontalAlignment::Center);
        }

        // ── Drag insertion marker ──
        if let Some(target) = self.drag_target {
            if let Some((_, rect)) =
                self.cached_layout().chips.iter().find(|(i, _)| *i == target).copied()
            {
                let marker = Rect::new(rect.x.saturating_sub(2), rect.y, 2, rect.height);
                context.fill_rect(marker, text_color.with_alpha(200));
            }
        }

        // ── Sliding selection pill ──
        //
        // The motion is a *travelling highlight*, drawn under no chip (it is painted after
        // them, so it reads as an outline). It exists only between the moment a selection
        // moves and the frame its progress settles, which is why it can never alter a resting
        // snapshot.
        if self.animate_selection && self.pill_slide.is_moving() {
            let layout = &self.cached_layout().chips;
            let selected = self.items.iter().position(|item| item.selected);
            if let (Some(from), Some(to)) = (
                layout.iter().find(|(i, _)| *i == self.pill_from),
                selected.and_then(|index| layout.iter().find(|(i, _)| *i == index)),
            ) {
                let t = self.pill_slide.value();
                let x = from.1.x as f32 + (to.1.x - from.1.x) as f32 * t;
                let y = from.1.y as f32 + (to.1.y - from.1.y) as f32 * t;
                let w = from.1.width as f32 + (to.1.width as f32 - from.1.width as f32) * t;
                let pill = Rect::new(
                    x.round() as i32,
                    y.round() as i32,
                    w.round().max(1.0) as u32,
                    to.1.height,
                );
                context.draw_rounded_rect_stroke(
                    pill,
                    (pill.height / 2).max(1),
                    text_color.with_alpha(120),
                    1,
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

    fn sample_items() -> Vec<ChipItem> {
        vec![
            ChipItem::new("bug", "Bug"),
            ChipItem::new("feature", "Feature"),
            ChipItem::new("urgent", "Urgent"),
        ]
    }

    #[test]
    fn single_select_keeps_only_one_selected() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());

        assert!(chip.toggle_index(0));
        assert_eq!(chip.selected_ids(), vec!["bug"]);

        assert!(chip.toggle_index(1));
        assert_eq!(chip.selected_ids(), vec!["feature"]);
    }

    #[test]
    fn multi_select_allows_multiple_selected() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());
        chip.set_multi_select(true);

        assert!(chip.toggle_index(0));
        assert!(chip.toggle_index(2));
        assert_eq!(chip.selected_ids(), vec!["bug", "urgent"]);
    }

    #[test]
    fn chip_toggled_emits_toggled_id() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());

        let emitted = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = emitted.clone();
        chip.chip_toggled.connect(move |id| {
            if let Ok(mut guard) = sink.lock() {
                guard.push(id.as_ref().clone());
            }
        });

        assert!(chip.toggle_index(2));
        let got = emitted.lock().ok().map(|guard| guard.clone()).unwrap_or_default();
        assert_eq!(got, vec!["urgent".to_string()]);
    }

    #[test]
    fn default_state() {
        let chip = Chip::new(Rect::new(0, 0, 800, 600));
        assert!(chip.items().is_empty());
        assert_eq!(chip.focused_index(), None);
        assert!(!chip.multi_select());
        assert!(chip.selected_ids().is_empty());
    }

    #[test]
    fn set_items_adds_chips() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_items(vec![ChipItem::new("a", "Alpha"), ChipItem::new("b", "Beta")]);

        assert_eq!(chip.items().len(), 2);
        assert_eq!(chip.items()[0].id, "a");
        assert_eq!(chip.items()[1].label, "Beta");
        assert_eq!(chip.focused_index(), Some(0));
    }

    #[test]
    fn empty_items_state() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_items(Vec::new());

        assert_eq!(chip.focused_index(), None);
        assert!(chip.selected_ids().is_empty());

        // Toggle out of bounds returns false
        assert!(!chip.toggle_index(0));
        assert!(!chip.toggle_index(100));

        // Move focus on empty should not panic
        chip.move_focus(1);
        assert_eq!(chip.focused_index(), None);

        chip.move_focus(-1);
        assert_eq!(chip.focused_index(), None);
    }

    #[test]
    fn invalid_toggle_index() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_items(vec![ChipItem::new("c1", "Chip 1")]);

        // Out of bounds returns false
        assert!(!chip.toggle_index(5));

        // Valid toggle works
        assert!(chip.toggle_index(0));
        assert_eq!(chip.selected_ids(), vec!["c1"]);
    }

    /// A-4: selecting one chip changes **that chip's box**, not the whole row band.
    ///
    /// # The defect this pins
    ///
    /// The row band took the resolved `chip:selected` fill, because `background` was read from the
    /// already-selected `style` — so selecting a single chip repainted the entire row in `primary`
    /// and derived every other chip's fill from it. The theme's declared `chip:selected` value is
    /// the selected chip's own appearance, and the row is the base `chip` appearance.
    ///
    /// # How the assertion isolates the row
    ///
    /// Two chips share one row band. The row's leftmost pixel column lies inside the band but
    /// outside the first chip's own box only if the chip starts inset from it; the band's own fill
    /// is therefore probed on the band's outer edge, where no chip is drawn. Under the defect that
    /// pixel took the `primary` tint; with the fix it stays the base `chip` background.
    #[test]
    fn selecting_one_chip_does_not_repaint_the_row_band() {
        use crate::render::{PaintBackend, SoftwarePaintBackend};
        use crate::widget::draw::Draw;

        let bounds = Rect::new(0, 0, 300, 36);
        // The declared `chip:selected` key lives in the shipped preset, so the test must select
        // that theme for the key to resolve at all.
        let _guard = crate::theme::theme_test_guard();
        {
            let mut manager = crate::theme::global_theme_manager();
            assert!(manager.set_theme("default"));
        }
        let mut chip = Chip::new(bounds);
        chip.set_items(vec![ChipItem::new("a", "A"), ChipItem::new("b", "B")]);

        let read_band_pixel = |chip: &mut Chip| -> crate::core::Color {
            let mut surface = SoftwarePaintBackend::new(bounds.size(), 1.0);
            surface.begin_frame(crate::core::Color::WHITE);
            {
                let mut context = crate::render::RenderContext::new(&mut surface);
                chip.draw(&mut context);
            }
            surface.end_frame();
            // The band is a full-width strip centred in the control, and chip boxes begin
            // `CHIP_PADDING_H` in from its leading edge; the band's own leftmost columns are
            // therefore never covered by a chip.
            let band = chip.row_band();
            let frame = surface.frame_rgba();
            let probe_x = 2usize;
            let offset = ((band.y as usize + 2) * bounds.width as usize + probe_x) * 4;
            let pixel = &frame[offset..offset + 4];
            crate::core::Color::rgba(pixel[0], pixel[1], pixel[2], pixel[3])
        };

        let unselected = read_band_pixel(&mut chip);
        assert!(chip.toggle_index(0), "selecting the first chip must succeed");
        let selected = read_band_pixel(&mut chip);

        assert_eq!(
            unselected, selected,
            "selecting one chip must not change the row band's own fill"
        );

        // The stronger statement: the band is the **base** `chip` appearance, not the declared
        // selected fill. Comparing the two readings alone would pass if the band took the
        // selected colour in *both* (the band read the style before any chip was chosen too).
        let declared_selected = crate::style::resolved_theme_style_for_state(
            "chip",
            crate::style::WidgetState::Selected,
        )
        .and_then(|resolved| resolved.background_color)
        .expect("the shipped preset declares `chip:selected`");
        assert_ne!(
            selected, declared_selected,
            "the row band must not take the selected chip's declared fill"
        );
    }

    #[test]
    fn multi_select_toggle() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_items(vec![
            ChipItem::new("a", "A"),
            ChipItem::new("b", "B"),
            ChipItem::new("c", "C"),
        ]);

        // Enable multi-select after set_items - should preserve no selection
        chip.set_multi_select(true);
        assert!(chip.multi_select());

        assert!(chip.toggle_index(0));
        assert!(chip.toggle_index(2));
        assert_eq!(chip.selected_ids(), vec!["a", "c"]);

        // Toggle one off
        assert!(chip.toggle_index(0));
        assert_eq!(chip.selected_ids(), vec!["c"]);
    }

    #[test]
    fn set_multi_select_downgrade_preserves_one() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_multi_select(true);
        chip.set_items(vec![ChipItem::new("a", "A"), ChipItem::new("b", "B")]);

        assert!(chip.toggle_index(0));
        assert!(chip.toggle_index(1));
        assert_eq!(chip.selected_ids().len(), 2);

        // Switch to single-select
        chip.set_multi_select(false);
        assert!(!chip.multi_select());
        // Should keep only the last selected
        let ids = chip.selected_ids();
        assert_eq!(ids.len(), 1, "single-select must keep at most one selected");
    }

    #[test]
    fn keyboard_focus_and_toggle() {
        let mut chip = Chip::new(Rect::new(0, 0, 800, 600));
        chip.set_items(vec![
            ChipItem::new("a", "A"),
            ChipItem::new("b", "B"),
            ChipItem::new("c", "C"),
        ]);

        // Move right
        chip.handle_event(&Event::key_press(39, 0));
        assert_eq!(chip.focused_index(), Some(1));

        // Toggle with Enter
        chip.handle_event(&Event::key_press(13, 0));
        assert_eq!(chip.selected_ids(), vec!["b"]);

        // Move left
        chip.handle_event(&Event::key_press(37, 0));
        assert_eq!(chip.focused_index(), Some(0));

        // Toggle with Space
        chip.handle_event(&Event::key_press(32, 0));
        assert_eq!(chip.selected_ids(), vec!["a"]);
    }

    /// A chip's height is chrome, not a fraction of the control.
    ///
    /// The defect this pins: the chip's height was `rect.height - 8`, so a 240x120 census
    /// cell drew a 112 px chip while a chip in a 32 px toolbar drew a 24 px one — the same
    /// control at two sizes. A chip's height is its own, so it is `CHIP_HEIGHT` whenever the
    /// control has room for it and clamped to the control only when it does not, which is
    /// the same "never paint outside the rectangle" rule every other piece of chrome follows.
    #[test]
    fn a_chip_is_the_same_height_in_any_rectangle() {
        for height in [32u32, 48, 120, 320] {
            let mut chip = Chip::new(Rect::new(0, 0, 240, height));
            chip.set_items(vec![ChipItem::new("a", "A")]);
            let rect = chip.chip_rect(0).expect("one item has one chip");
            assert_eq!(rect.height, dimensions::CHIP_HEIGHT, "at control height {height}");
        }
        // A control shorter than a chip clamps it rather than painting outside.
        let mut short = Chip::new(Rect::new(0, 0, 240, 20));
        short.set_items(vec![ChipItem::new("a", "A")]);
        let rect = short.chip_rect(0).expect("one item has one chip");
        assert_eq!(rect.height, 20);
    }

    /// The chip row never paints outside the rectangle it was given.
    #[test]
    fn the_chip_row_stays_inside_a_short_control() {
        let mut chip = Chip::new(Rect::new(0, 0, 240, 16));
        chip.set_items(vec![ChipItem::new("a", "A")]);
        let rect = chip.chip_rect(0).expect("one item has one chip");
        assert!(rect.y >= 0, "the chip must start inside the control");
        assert!(rect.y + rect.height as i32 <= 16, "the chip must end inside the control");
    }

    /// A chip list with a chosen member reports `Selected` — its own state, not `Checked`.
    ///
    /// # Why this is `Selected` and not `Checked`
    ///
    /// The preset carries a `selected` key for the state that means "one of the currently selected
    /// items in a collection" and a `checked` key for "a toggle-like control is on". This control
    /// is a **collection**: several of its members can be chosen at once, and there is no single
    /// binary "on" for the list as a whole. Reporting `Checked` would claim a shape the control
    /// does not have.
    ///
    /// The defect was the same as the latching controls', one level down: the trait default knows
    /// only the four primitive flags, so **neither** key was ever reached from here.
    #[test]
    fn widget_state_reports_selected_for_a_chip_list_with_a_chosen_member() {
        use crate::style::WidgetState;
        let mut chip = Chip::new(Rect::new(0, 0, 240, 16));
        chip.set_items(vec![ChipItem::new("a", "A"), ChipItem::new("b", "B")]);
        assert_eq!(chip.widget_state(), WidgetState::Normal);

        // Selection is a property of an *item*, so the report follows the collection.
        let mut items = chip.items().to_vec();
        items[1].selected = true;
        chip.set_items(items);
        assert_eq!(chip.widget_state(), WidgetState::Selected);

        chip.handle_event(&Event::MouseEnter { pos: Point::new(1, 1) });
        assert_eq!(
            chip.widget_state(),
            WidgetState::Selected,
            "a chosen member outranks the list's own hover"
        );
    }

    // ── B26.09: the item list as a typed collection property ──

    /// The `items` property round-trips the live items, and identity is what it round-trips *by*.
    ///
    /// Before B26.09 the item list was reachable only as `set_items`, whose argument the property API
    /// could not carry: `items` was not a property name at all, so a document's `"items": [..]` had
    /// nowhere to go.
    #[test]
    fn the_items_property_round_trips_the_live_items() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());
        chip.set_multi_select(true);
        assert!(chip.toggle_index(1));

        let CapabilityValue::Records(list) = chip.get("items").expect("readable") else {
            panic!("`items` must read as a record list");
        };
        assert_eq!(list.len(), 3);
        assert_eq!(list.schema().id_field, "id");
        assert_eq!(list.id_of(0), Some("bug"));
        assert_eq!(list.get_by_id("urgent").expect("present").text("label"), "Urgent");
        assert!(matches!(
            list.get_by_id("feature").expect("present").get("selected"),
            Some(CapabilityValue::Bool(true))
        ));
        assert!(matches!(
            list.get_by_id("bug").expect("present").get("selected"),
            Some(CapabilityValue::Bool(false))
        ));
    }

    /// A row move survives a write/read round trip: the item keeps its id and its own fields, and only
    /// its position changes.
    #[test]
    fn a_moved_item_keeps_its_identity_through_a_property_write() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());
        chip.set_multi_select(true);
        assert!(chip.toggle_index(0));

        let CapabilityValue::Records(mut list) = chip.get("items").expect("readable") else {
            panic!("`items` must read as a record list");
        };
        // Move the *selected* item to the end: its selection must travel with it, and the selection is
        // held by id, so a position-based implementation would lose it.
        list.move_item(0, 2).expect("both indices are in range");
        chip.set("items", CapabilityValue::Records(list)).expect("a conforming list is accepted");

        let ids: Vec<&str> = chip.items().iter().map(|item| item.id.as_str()).collect();
        assert_eq!(ids, vec!["feature", "urgent", "bug"]);
        assert_eq!(chip.selected_ids(), vec!["bug"]);
    }

    /// A runtime-only item icon is **refused** by the read, not quietly left out.
    ///
    /// §19.13 case I13 lets a non-serialisable source be either faithful or refused, but requires the
    /// result to be clear — and a record that simply *omits* `icon_source` cannot be told apart from
    /// one whose chip has no icon. A designer round-trip would therefore drop the icon and the
    /// following write would erase it. The control-level `icon_source` property refuses the same
    /// value with the same error, which is the rule asserted here.
    #[test]
    fn a_runtime_only_item_icon_is_refused_by_the_read_rather_than_dropped() {
        use crate::widget::display_widgets::icon::IconData;
        use crate::widget::icon_source::IconSource;
        use crate::widget::IconName;

        static SOLID: IconData =
            IconData { name: "chip-solid", grid: 24, paths: &["M4-4H20V-20H4Z"] };

        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());
        chip.set_item_icon_source("bug", Some(IconSource::Builtin(IconName::Info)))
            .expect("the chip exists");

        // A persistable item icon still reads back, so the refusal below is about the source and not
        // about the channel being inert.
        let CapabilityValue::Records(list) = chip.get("items").expect("a built-in reads back")
        else {
            panic!("`items` must read as a record list");
        };
        assert!(matches!(
            list.get_by_id("bug").expect("present").get("icon_source"),
            Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                IconName::Info
            )))
        ));

        chip.set_item_icon_source("bug", Some(IconSource::static_geometry(&SOLID)))
            .expect("the chip exists");
        assert_eq!(
            chip.get("items"),
            Err(CapabilityAccessError::IconSourceNotPersistable),
            "a source with no document form must refuse the read rather than lose the icon"
        );
    }

    /// A malformed list is refused **and the live items are untouched** — the whole list is checked
    /// before anything is replaced.
    ///
    /// This is what makes a withdrawn or superseded pending edit safe to abandon: there is no partial
    /// application to roll back.
    #[test]
    fn a_malformed_record_list_is_refused_and_changes_nothing() {
        use crate::widget::capability::{RecordList, RecordValue, EMPTY_RECORD_SCHEMA};

        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());
        chip.set_multi_select(true);
        assert!(chip.toggle_index(1));
        let before = chip.items().to_vec();
        let selection_before: Vec<String> =
            chip.selected_ids().into_iter().map(String::from).collect();

        // Each entry below breaks exactly one rule of `CHIP_ITEM_SCHEMA`.
        let broken: Vec<(&str, Vec<RecordValue>)> = vec![
            (
                "missing label",
                vec![RecordValue::new().with("id", CapabilityValue::String(String::from("a")))],
            ),
            (
                "unknown field",
                vec![RecordValue::new()
                    .with("id", CapabilityValue::String(String::from("a")))
                    .with("label", CapabilityValue::String(String::from("A")))
                    .with("weight", CapabilityValue::UInt(1))],
            ),
            (
                "wrong kind",
                vec![RecordValue::new()
                    .with("id", CapabilityValue::UInt(1))
                    .with("label", CapabilityValue::String(String::from("A")))],
            ),
            (
                "duplicate id",
                vec![
                    RecordValue::new()
                        .with("id", CapabilityValue::String(String::from("a")))
                        .with("label", CapabilityValue::String(String::from("A"))),
                    RecordValue::new()
                        .with("id", CapabilityValue::String(String::from("a")))
                        .with("label", CapabilityValue::String(String::from("B"))),
                ],
            ),
            (
                "empty id",
                vec![RecordValue::new()
                    .with("id", CapabilityValue::String(String::new()))
                    .with("label", CapabilityValue::String(String::from("A")))],
            ),
        ];

        for (label, records) in broken {
            let list = RecordList::from_items(&EMPTY_RECORD_SCHEMA, records);
            // The refusal names the property and the reason, so a designer can point at the field
            // and say why — a bare `TypeMismatch` could not say which record or which field.
            match chip.set("items", CapabilityValue::Records(list)) {
                Err(CapabilityAccessError::ConstraintViolation { property, reason }) => {
                    assert_eq!(property, "items", "`{label}` must name the property");
                    assert!(!reason.is_empty(), "`{label}` must state a reason");
                }
                other => panic!("`{label}` must be refused with a reason, got {other:?}"),
            }
            assert_eq!(chip.items(), before.as_slice(), "`{label}` must not change the items");
            let selection_after: Vec<String> =
                chip.selected_ids().into_iter().map(String::from).collect();
            assert_eq!(selection_after, selection_before, "`{label}` must not change selection");
        }

        // A non-list value is refused for the same reason, and by the same contract.
        assert_eq!(
            chip.set("items", CapabilityValue::String(String::from("[]"))),
            Err(CapabilityAccessError::TypeMismatch)
        );
        assert_eq!(chip.items(), before.as_slice());
    }

    /// An accepted list normalises exactly the way `set_items` does, because the property write goes
    /// through the control's own setter rather than restating its rules.
    #[test]
    fn an_accepted_list_is_normalised_by_the_controls_own_setter() {
        use crate::widget::capability::{RecordList, RecordValue, EMPTY_RECORD_SCHEMA};

        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        // Single-select: two records claiming `selected` must collapse to one, which is
        // `set_items`'s normalisation and not a rule the property path may skip.
        let list = RecordList::from_items(
            &EMPTY_RECORD_SCHEMA,
            vec![
                RecordValue::new()
                    .with("id", CapabilityValue::String(String::from("a")))
                    .with("label", CapabilityValue::String(String::from("A")))
                    .with("selected", CapabilityValue::Bool(true)),
                RecordValue::new()
                    .with("id", CapabilityValue::String(String::from("b")))
                    .with("label", CapabilityValue::String(String::from("B")))
                    .with("selected", CapabilityValue::Bool(true)),
            ],
        );
        chip.set("items", CapabilityValue::Records(list)).expect("conforming");
        assert_eq!(chip.selected_ids(), vec!["a"], "single-select keeps only the first claimant");

        // An omitted `selected` means "not selected", which is the state `ChipItem::new` starts in.
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        let list = RecordList::from_items(
            &EMPTY_RECORD_SCHEMA,
            vec![RecordValue::new()
                .with("id", CapabilityValue::String(String::from("a")))
                .with("label", CapabilityValue::String(String::from("A")))],
        );
        chip.set("items", CapabilityValue::Records(list)).expect("`selected` is optional");
        assert_eq!(chip.selected_ids(), Vec::<&str>::new());
        assert_eq!(chip.items().len(), 1);
    }

    // ── Item icons: identity-keyed, drawn, and carried by the record ──

    /// A chip item's typed icon is addressed by the item's **stable id**, and a missing or
    /// duplicated id is reported rather than silently targeting the wrong chip.
    #[test]
    fn a_chip_item_icon_is_addressed_by_id_and_drawn() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(vec![ChipItem::new("bug", "Bug"), ChipItem::new("fix", "Fix")]);
        let render = |chip: &mut Chip| crate::widget::svg::render_to_svg(chip);
        let without = render(&mut chip);

        assert_eq!(
            chip.set_item_icon_source(
                "nope",
                Some(IconSource::Builtin(crate::widget::IconName::Info))
            ),
            Err(crate::widget::IconItemError::UnknownItemId { id: String::from("nope") })
        );
        chip.set_item_icon_source("bug", Some(IconSource::Builtin(crate::widget::IconName::Info)))
            .expect("the chip exists");

        let with = render(&mut chip);
        assert_eq!(
            with.matches("<path").count(),
            without.matches("<path").count() + 1,
            "a chip item icon must reach the drawing backend exactly once"
        );
        // The icon is part of the chip's box, so the row is measured again -- an icon that reserved
        // no room would overlap its own label.
        assert!(
            chip.chip_rect(0).expect("the chip exists").width
                > chip.chip_rect(1).expect("the chip exists").width,
            "the icon must widen the chip that carries it"
        );

        chip.set_item_icon_source("bug", None).expect("the chip exists");
        assert!(chip.item_icon_source("bug").is_none());
        assert_eq!(render(&mut chip).matches("<path").count(), without.matches("<path").count());
    }

    /// `items` carries the icon, so a document round-trips it without a side channel.
    #[test]
    fn the_items_record_carries_the_item_icon_and_replaces_it() {
        use crate::widget::capability::{CapabilityValue, RecordList, RecordValue};

        let run = |chip: &mut Chip| {
            chip.set_items(vec![ChipItem::new("bug", "Bug")]);
            chip.set_item_icon_source(
                "bug",
                Some(IconSource::Builtin(crate::widget::IconName::Info)),
            )
            .expect("the chip exists");
            match chip.get("items").expect("readable") {
                CapabilityValue::Records(list) => list,
                other => panic!("expected records, got {other:?}"),
            }
        };
        let chip = &mut Chip::new(Rect::new(0, 0, 300, 36));
        let list = run(chip);
        assert!(matches!(
            list.items()[0].get("icon_source"),
            Some(CapabilityValue::IconSpec(crate::widget::icon_source::IconSpec::Builtin(
                crate::widget::IconName::Info
            )))
        ));

        // A document that omits the field means "no icon here", so applying a plain list clears the
        // override rather than leaving the old one in place.
        let plain = RecordList::from_items(
            &crate::widget::capability::EMPTY_RECORD_SCHEMA,
            vec![RecordValue::new()
                .with("id", CapabilityValue::String(String::from("bug")))
                .with("label", CapabilityValue::String(String::from("Bug")))],
        );
        chip.set("items", CapabilityValue::Records(plain)).expect("a plain row is valid");
        assert!(chip.item_icon_source("bug").is_none(), "an omitted field clears the override");
    }

    /// `items` is part of the published contract, and the capability's own table declares it as a
    /// record list — so a designer that reads the schema learns the field shape rather than guessing.
    #[test]
    fn the_items_property_is_published_with_its_record_schema() {
        let chip = Chip::new(Rect::new(0, 0, 300, 36));
        assert!(
            WidgetProperties::property_names(&chip).contains(&"items"),
            "the published contract must name `items`, not only the `set_items` command"
        );
        let schema = crate::widget::capability::CHIP_PROPERTIES
            .iter()
            .find(|schema| schema.name == "items")
            .expect("`items` is declared by the capability table");
        assert_eq!(schema.value_kind, crate::widget::capability::PropertyValueKind::Records);
        assert!(schema.readable && schema.writable);
        let record_schema = schema.record_schema.expect("a `Records` schema names its shape");
        assert_eq!(record_schema.id_field, "id");
        assert_eq!(
            record_schema.names(),
            vec!["id", "label", "selected", "icon_source", "avatar", "enabled", "closable"]
        );
        assert!(record_schema.ids_unique && record_schema.ids_required);
        // The control itself carries the same schema, so the two cannot drift.
        assert_eq!(record_schema, &CHIP_ITEM_SCHEMA);
    }

    // ── D09-CTRL-02: the chip box is measured with the shared text ruler ──

    /// A wide-glyph (CJK) chip must be wide enough for its label rather than truncated early.
    ///
    /// # The defect this pins
    ///
    /// `chip_width` estimated `chars().count() * 8`, which counts a CJK scalar as one narrow cell.
    /// The real advance is far wider, so the chip box was narrower than the text it drew and the
    /// label was fitted/truncated far sooner than necessary. The box now uses the shared
    /// `estimate_text_width`, so it fits the label it paints, and the click target is that same box.
    #[test]
    fn a_wide_glyph_chip_is_wide_enough_for_its_label() {
        let label = "\u{4e2d}\u{6587}"; // two CJK scalars
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(vec![ChipItem::new("a", label)]);

        let measured = estimate_text_width(label, &Font::default(), 1.0) as i32;
        let old_estimate = label.chars().count() as i32 * 8;
        assert!(
            measured > old_estimate,
            "CJK advances wider than the old per-char estimate (was {old_estimate}, measured {measured})"
        );

        let rect = chip.chip_rect(0).expect("one item has one chip");
        assert!(
            rect.width as i32 >= measured + chip.chip_padding * 2,
            "the chip box must hold the measured label plus its padding, got width {}",
            rect.width
        );

        // The click target is that same box, so a press in its right-hand part still hits.
        let inside = Point::new(rect.x + rect.width as i32 - 1, rect.y + rect.height as i32 / 2);
        assert_eq!(chip.hit_index(inside), Some(0));
    }

    /// The summary cell is a control: pressing it reports how many chips are hidden.
    #[test]
    fn pressing_the_overflow_summary_emits_the_hidden_count() {
        use crate::event::EventHandler;
        let mut chip = Chip::new(Rect::new(0, 0, 90, 24));
        chip.set_items(
            (0..6).map(|index| ChipItem::new(format!("c{index}"), "Wide label")).collect(),
        );
        let (_, overflow) = chip.chip_layout_with_overflow();
        let (cell, hidden) = overflow.expect("a summary cell when the row overflows");
        let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::<usize>::new()));
        let sink = std::sync::Arc::clone(&seen);
        chip.overflow_activated.connect(move |value| sink.lock().unwrap().push(*value));

        chip.handle_event(&Event::mouse_press(cell.x + 2, cell.y + 2, 1));
        assert_eq!(*seen.lock().unwrap(), vec![hidden]);
    }

    /// A chip avatar survives the `items` property round trip in all three forms.
    #[test]
    fn a_chip_avatar_round_trips_through_the_items_property() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 24));
        chip.set_items(vec![
            ChipItem::new("a", "Alice").with_avatar(ChipAvatar::Initials("A".to_string())),
            ChipItem::new("b", "Bob").with_avatar(ChipAvatar::Color(Color::rgb(10, 20, 30))),
            ChipItem::new("c", "Carol").with_avatar(ChipAvatar::Icon(IconName::User)),
        ]);

        let records = match chip.get("items").expect("items is readable") {
            CapabilityValue::Records(records) => records,
            other => panic!("items should read back as Records, got {other:?}"),
        };
        assert!(records.items()[0].get("avatar").is_some());

        chip.set("items", CapabilityValue::Records(records)).expect("a read-back list is writable");
        assert_eq!(chip.items()[0].avatar, Some(ChipAvatar::Initials("A".to_string())));
        assert_eq!(chip.items()[1].avatar, Some(ChipAvatar::Color(Color::rgb(10, 20, 30))));
        assert_eq!(chip.items()[2].avatar, Some(ChipAvatar::Icon(IconName::User)));
    }

    #[test]
    fn overflow_folds_into_a_summary_cell() {
        let mut chip = Chip::new(Rect::new(0, 0, 120, 36));
        chip.set_items(vec![
            ChipItem::new("a", "A very long chip label"),
            ChipItem::new("b", "Another very long chip label"),
            ChipItem::new("c", "Third"),
        ]);
        let (layout, overflow) = chip.chip_layout_with_overflow();
        assert!(overflow.is_some(), "the third chip must fold into a summary");
        assert!(layout.len() < 3);
    }

    #[test]
    fn close_removes_the_chip_through_the_same_path_as_delete() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());
        chip.set_show_close(true);
        let rect = chip.chip_rect(0).expect("the first chip is laid out");
        let close = Point::new(rect.x + rect.width as i32 - 2, rect.y + rect.height as i32 / 2);
        chip.handle_event(&Event::MousePress { pos: close, button: 1, modifiers: 0 });
        assert_eq!(chip.items().len(), 2);
        assert_eq!(chip.items()[0].id, "feature");
    }

    #[test]
    fn reordering_moves_the_chip_and_publishes_its_id() {
        use std::sync::{Arc, Mutex};
        let mut chip = Chip::new(Rect::new(0, 0, 300, 36));
        chip.set_items(sample_items());
        let moved = Arc::new(Mutex::new(Vec::<String>::new()));
        let sink = Arc::clone(&moved);
        chip.chip_reordered.connect(move |id| sink.lock().unwrap().push(id.to_string()));
        assert!(chip.move_item(0, 2));
        assert_eq!(chip.items()[2].id, "bug");
        assert_eq!(*moved.lock().unwrap(), vec!["bug".to_string()]);
    }
}

#[cfg(test)]
mod issue_2_8_tests {
    use super::*;
    use crate::event::Event;

    /// SP-18: a disabled item refuses toggling while its neighbours keep working.
    #[test]
    fn a_disabled_item_refuses_toggling() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 32));
        chip.set_items(vec![
            ChipItem::new("a", "A").with_enabled(false),
            ChipItem::new("b", "B"),
        ]);
        assert!(!chip.toggle_index(0));
        assert!(chip.selected_ids().is_empty());
        assert!(chip.toggle_index(1));
        assert_eq!(chip.selected_ids(), ["b"]);
    }

    /// SP-18: the per-item close override wins over the control-level switch.
    #[test]
    fn per_item_close_override_is_stored_and_read_back() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 32));
        chip.set_items(vec![ChipItem::new("a", "A"), ChipItem::new("b", "B")]);
        assert!(chip.set_item_closable("b", false));
        assert_eq!(chip.items()[1].closable, Some(false));
        assert!(!chip.set_item_closable("missing", true));
        // The record round-trip keeps the override.
        let read = WidgetProperties::get(&chip, "items").expect("items is readable");
        let CapabilityValue::Records(list) = read else {
            panic!("items is a record list");
        };
        let record = list.items().last().expect("two records");
        assert_eq!(record.get("closable"), Some(&CapabilityValue::Bool(false)));
    }

    /// SP-20: Home and End reach the row's ends.
    #[test]
    fn home_and_end_move_focus_to_the_row_edges() {
        let mut chip = Chip::new(Rect::new(0, 0, 300, 32));
        chip.set_items(vec![
            ChipItem::new("a", "A"),
            ChipItem::new("b", "B"),
            ChipItem::new("c", "C"),
        ]);
        chip.move_focus(1);
        chip.handle_event(&Event::key_press(35, 0));
        assert_eq!(chip.focused_index(), Some(2));
        chip.handle_event(&Event::key_press(36, 0));
        assert_eq!(chip.focused_index(), Some(0));
    }

    /// SP-17: wrapping flows onto further rows and stops folding into `+N`.
    #[test]
    fn wrapping_flows_onto_further_rows() {
        let mut chip = Chip::new(Rect::new(0, 0, 120, 80));
        chip.set_items(
            (0..6)
                .map(|index| ChipItem::new(format!("i{index}"), format!("Item {index}")))
                .collect(),
        );
        chip.set_wrap(true);
        let (chips, overflow) = chip.chip_layout_with_overflow();
        assert_eq!(chips.len(), 6);
        assert!(overflow.is_none());
        assert!(chips.iter().any(|(_, rect)| rect.y > 0), "a second row exists");
        assert!(chip.size_hint().height > dimensions::CHIP_HEIGHT);
    }

    /// COMP-H02: committing splits on the separator and honours the limit and duplicates rule.
    #[test]
    fn committing_splits_on_the_separator_and_enforces_the_rules() {
        use super::chip_input::ChipInput;
        let mut input = ChipInput::new(Rect::new(0, 0, 300, 80));
        input.set_text("a,b");
        assert_eq!(input.commit(), 2);
        assert_eq!(input.items().len(), 2);
        assert_eq!(input.text(), "");
        input.set_text("a");
        assert_eq!(input.commit(), 0, "a duplicate is refused");
        input.set_max_count(2);
        input.set_text("c");
        assert_eq!(input.commit(), 0, "the limit refuses the third chip");
        assert_eq!(input.items().len(), 2);
    }

    /// COMP-H02: backspace on an empty line removes the last chip.
    #[test]
    fn backspace_on_an_empty_line_removes_the_last_chip() {
        use super::chip_input::ChipInput;
        let mut input = ChipInput::new(Rect::new(0, 0, 300, 80));
        input.set_items(vec![ChipItem::new("a", "A"), ChipItem::new("b", "B")]);
        input.handle_event(&Event::key_press(8, 0));
        assert_eq!(input.items().len(), 1);
        assert_eq!(input.items()[0].id, "a");
    }
}
