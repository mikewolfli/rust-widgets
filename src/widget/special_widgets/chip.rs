// SPDX-FileCopyrightText: Copyright (c) 2026 Mike Li/Mikewolfli/Wei Li(mikewolfli@163.com)
// SPDX-License-Identifier: MIT

//! Chip widget.

use crate::core::{Color, HorizontalAlignment, Point, Rect};
use crate::event::{Event, EventHandler};
use crate::render::RenderContext;
use crate::signal::Signal1;
use crate::widget::capability::coercion::expect_bool;
use crate::widget::capability::properties_trait::{base_property_get, base_property_set};
use crate::widget::capability::types::{CapabilityAccessError, CapabilityValue};
use crate::widget::capability::WidgetProperties;
use crate::widget::metrics::{dimensions, effective_font, estimate_text_width, ControlMetrics};
use crate::widget::{BaseWidget, Draw, IconSource, Widget, WidgetKind};
use crate::{impl_widget_property_hooks, property_names_of};

/// One chip item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipItem {
    /// Stable item id.
    pub id: String,
    /// Display label.
    pub label: String,
    /// Whether the chip is selected.
    pub selected: bool,
}

impl ChipItem {
    /// Creates a new chip item.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self { id: id.into(), label: label.into(), selected: false }
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
        .with("selected", CapabilityValue::Bool(item.selected));
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
    Ok((ChipItem { id: text("id")?, label: text("label")?, selected }, icon_source))
}

/// Square side of a chip item's leading icon slot, in logical pixels.
const CHIP_ICON_SIDE: u32 = 14;
/// Gap between that slot and the chip's label.
const CHIP_ICON_GAP: u32 = 4;

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
    /// no longer exists (blue27 §19.6).
    item_icon_sources: Vec<(String, IconSource)>,
    /// Emitted when chip selection changes. Payload is chip id.
    pub chip_toggled: Signal1<String>,
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
            item_icon_sources: Vec::new(),
            chip_toggled: Signal1::new(),
        }
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
        self.item_icon_sources.retain(|(id, _)| live.contains(id));
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
        self.item_icon_sources.retain(|(existing, _)| existing != id);
        if let Some(source) = source {
            self.item_icon_sources.push((String::from(id), source));
        }
        // An icon is part of the chip's box, so the row has to be measured again, not just redrawn.
        self.base.request_layout();
        self.base.request_redraw();
        Ok(())
    }

    /// Returns the typed icon source assigned to the item owning `id`.
    pub fn item_icon_source(&self, id: &str) -> Option<IconSource> {
        self.item_icon_sources
            .iter()
            .find(|(existing, _)| existing == id)
            .map(|(_, source)| *source)
    }

    /// Returns the typed icon source for `item`, if one is assigned.
    fn icon_source_of(&self, item: &ChipItem) -> Option<IconSource> {
        self.item_icon_source(&item.id)
    }

    /// Replaces the whole id-keyed override map, dropping any entry the new list does not name.
    fn adopt_icon_sources(&mut self, sources: Vec<(String, IconSource)>) {
        self.item_icon_sources = sources;
        let live: Vec<String> = self.items.iter().map(|item| item.id.clone()).collect();
        self.item_icon_sources.retain(|(id, _)| live.contains(id));
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
        self.chip_toggled.emit(id);
        self.base.request_redraw();
        true
    }

    /// Moves focus by signed delta.
    pub fn move_focus(&mut self, delta: isize) {
        if self.items.is_empty() {
            self.focused_index = None;
            return;
        }
        let current = self.focused_index.unwrap_or(0) as isize;
        let max = self.items.len().saturating_sub(1) as isize;
        let next = (current + delta).clamp(0, max) as usize;
        self.focused_index = Some(next);
        self.base.request_redraw();
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
    fn chip_width(&self, item: &ChipItem, padding: i32) -> i32 {
        // The **effective font** — the resolved theme/caller font — so the chip box and the label are
        // measured with the same font the row paints with (D09-STYLE-01 / D09-STYLE-02).
        let icon =
            if self.icon_source_of(item).is_some() { CHIP_ICON_SIDE + CHIP_ICON_GAP } else { 0 };
        estimate_text_width(&item.label, effective_font(self.style()), 1.0) as i32
            + padding * 2
            + icon as i32
    }

    /// The band the chip row occupies: full width, `CHIP_HEIGHT` tall, centred in the
    /// control's rectangle.
    ///
    /// # Why the row has its own height
    ///
    /// A chip is chrome: it is the same height whoever hands it the row. Deriving it from
    /// `rect.height - 8` made a 240x120 census cell draw a **112 px chip** — a column shaped
    /// like a chip rather than a chip — and made a chip inside a 64 px toolbar a different
    /// object from one inside a 120 px cell. [`ControlMetrics::full_width_band`] is the
    /// shared derivation for "my width, my own height", so the drawn chip and the 24 px
    /// `size_hint` can no longer describe different controls.
    fn row_band(&self) -> Rect {
        ControlMetrics::full_width_band(self.geometry(), dimensions::CHIP_HEIGHT)
    }

    fn chip_rect(&self, index: usize) -> Option<Rect> {
        let band = self.row_band();
        let mut x = band.x + dimensions::CHIP_PADDING_H as i32;
        for (i, item) in self.items.iter().enumerate() {
            let width = self.chip_width(item, self.chip_padding).max(10);
            if i == index {
                // Only the width is content-driven; the height is the chip's own, so the
                // chip sits in the row band rather than at the control's top edge.
                let width = (width as u32).min(band.width);
                return Some(Rect::new(x, band.y, width, band.height));
            }
            x += width + self.chip_spacing;
        }
        None
    }

    fn hit_index(&self, pos: Point) -> Option<usize> {
        for index in 0..self.items.len() {
            let Some(chip) = self.chip_rect(index) else {
                continue;
            };
            if pos.x >= chip.x
                && pos.x < chip.x + chip.width as i32
                && pos.y >= chip.y
                && pos.y < chip.y + chip.height as i32
            {
                return Some(index);
            }
        }
        None
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
        crate::core::Size::new(80, 24)
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
            Event::MousePress { pos, button: 1, .. } => {
                if let Some(index) = self.hit_index(*pos) {
                    self.focused_index = Some(index);
                    let _ = self.toggle_index(index);
                }
            }
            Event::KeyPress { key, modifiers: _ } => match *key {
                37 => self.move_focus(-1),
                39 => self.move_focus(1),
                13 | 32 => {
                    if let Some(index) = self.focused_index() {
                        let _ = self.toggle_index(index);
                    }
                }
                // Unknown key; ignore
                _ => {}
            },
            // Other events are not relevant for this widget
            _ => {}
        }
    }
}

impl Draw for Chip {
    fn draw(&mut self, context: &mut RenderContext) {
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
        let theme = crate::style::resolved_theme_style("chip");
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

        for index in 0..self.items.len() {
            let Some(chip_rect) = self.chip_rect(index) else {
                continue;
            };

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
            let mut bg = if item.selected {
                background.blend(&text_color, 0.22)
            } else if self.focused_index == Some(index) {
                background.blend(&text_color, 0.12)
            } else {
                background.blend(&text_color, 0.06)
            };
            let mut label_ink = text_color;
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
            // ── Icon ──
            //
            // A chip with typed content reserves a leading square, and the label band starts after
            // it. `chip_width` charges exactly `CHIP_ICON_SIDE + CHIP_ICON_GAP`, so the room the
            // box was measured with and the room the glyph occupies cannot disagree.
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
            }
            // The chip's label is centred through the shared primitive: `chip_rect.y +
            // chip_rect.height / 2` is the glyph box's *top* edge on the chip's middle line,
            // which drew the label half a line low. The label itself is host-supplied, and the
            // chip's *box* is already clamped to the band (`(width as u32).min(band.width)`), so
            // the ink is fitted into that same clamped box: a long label now elides inside its
            // chip instead of running past the box the chip was drawn as.
            context.draw_text_line(
                label_box,
                &item.label,
                effective_font(style),
                label_ink,
                HorizontalAlignment::Left,
            );
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
        assert_eq!(record_schema.names(), vec!["id", "label", "selected", "icon_source"]);
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
}
